/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::net::SocketAddr;
use std::sync::Arc;

use log::{info, warn};
use tokio::net::TcpStream;
use tokio::sync::broadcast;

use g3_daemon::listen::{ListenAliveGuard, ListenStats};
use g3_daemon::server::ClientConnectionInfo;
use g3_io_ext::LimitedTcpListener;
use g3_socket::RawSocket;
use g3_types::ext::SocketAddrExt;
use g3_types::net::TcpListenConfig;

use super::{KeyServer, ServerReloadCommand};

pub(super) struct KeyServerRuntime {
    server: Arc<KeyServer>,
    listen_stats: Arc<ListenStats>,
    _alive_guard: Option<ListenAliveGuard>,
}

impl KeyServerRuntime {
    pub(crate) fn new(server: &Arc<KeyServer>) -> Self {
        KeyServerRuntime {
            server: Arc::clone(server),
            listen_stats: server.get_listen_stats(),
            _alive_guard: None,
        }
    }

    fn pre_start(&mut self) {
        info!("started SRT {}", self.server.name());
        self._alive_guard = Some(self.listen_stats.add_running_runtime());
    }

    fn pre_stop(&self) {
        info!("stopping SRT {}", self.server.name());
    }

    fn post_stop(&self) {
        info!("stopped SRT {}", self.server.name());
    }

    async fn run(
        self,
        mut listener: LimitedTcpListener,
        mut server_reload_channel: broadcast::Receiver<ServerReloadCommand>,
    ) {
        use broadcast::error::RecvError;

        loop {
            tokio::select! {
                biased;

                ev = server_reload_channel.recv() => {
                    match ev {
                        Ok(ServerReloadCommand::QuitRuntime) => {},
                        Err(RecvError::Closed) => {},
                        Err(RecvError::Lagged(dropped)) => {
                            warn!("SRT {} reload notify channel overflowed, {dropped} msg dropped",
                                self.server.name());
                            continue
                        },
                    }

                    info!("SRT {} will go offline", self.server.name());
                    self.pre_stop();
                    let accept_again = listener.set_offline();
                    if accept_again {
                        info!("SRT {} will accept all pending connections", self.server.name());
                        continue;
                    } else {
                        break;
                    }
                }
                result = listener.accept() => {
                    if listener.accept_current_available(result, |result| {
                        match result {
                            Ok(Some((stream, peer_addr, local_addr))) => {
                                self.listen_stats.add_accepted();
                                self.run_task(
                                    stream,
                                    peer_addr.to_canonical(),
                                    local_addr.to_canonical(),
                                );
                                Ok(())
                            }
                            Ok(None) => {
                                info!("SRT {} offline", self.server.name());
                                Err(())
                            }
                            Err(e) => {
                                self.listen_stats.add_failed();
                                warn!("SRT {} accept: {e:?}", self.server.name());
                                Ok(())
                            }
                        }
                    }).await.is_err() {
                        break;
                    }
                }
            }
        }
        self.post_stop();
    }

    fn run_task(&self, stream: TcpStream, peer_addr: SocketAddr, local_addr: SocketAddr) {
        let server = Arc::clone(&self.server);
        let mut cc_info = ClientConnectionInfo::new(peer_addr, local_addr);
        cc_info.set_tcp_raw_socket(RawSocket::from(&stream));
        tokio::spawn(async move {
            server.run_tcp_task(stream, cc_info).await;
        });
    }

    pub(super) fn into_running(
        mut self,
        listen_config: &TcpListenConfig,
        server_reload_sender: &broadcast::Sender<ServerReloadCommand>,
    ) -> anyhow::Result<()> {
        let listener = g3_socket::tcp::new_listen_to(listen_config)?;
        let server_reload_receiver = server_reload_sender.subscribe();
        tokio::spawn(async move {
            self.pre_start();
            self.run(LimitedTcpListener::new(listener), server_reload_receiver)
                .await
        });
        Ok(())
    }
}
