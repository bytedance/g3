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
use std::os::fd::AsRawFd;
use std::sync::Arc;

use async_trait::async_trait;
use log::{info, warn};
use tokio::net::TcpStream;
use tokio::runtime::Handle;
use tokio::sync::broadcast;

use g3_io_ext::LimitedTcpListener;
use g3_socket::util::native_socket_addr;
use g3_types::net::TcpListenConfig;

use crate::listen::ListenStats;
use crate::server::{BaseServer, ClientConnectionInfo, ServerReloadCommand};

#[async_trait]
pub trait AcceptTcpServer: BaseServer {
    async fn run_tcp_task(&self, stream: TcpStream, cc_info: ClientConnectionInfo);
}

pub trait ReloadTcpServer: AcceptTcpServer {
    fn get_reloaded(&self) -> Self;
}

#[derive(Clone)]
pub struct ListenTcpRuntime<S> {
    server: S,
    server_type: &'static str,
    server_version: usize,
    worker_id: Option<usize>,
    listen_stats: Arc<ListenStats>,
    instance_id: usize,
}

impl<S> ListenTcpRuntime<S>
where
    S: ReloadTcpServer + Clone + Send + Sync + 'static,
{
    pub fn new(server: S, listen_stats: Arc<ListenStats>) -> Self {
        let server_type = server.server_type();
        let server_version = server.version();
        ListenTcpRuntime {
            server,
            server_type,
            server_version,
            worker_id: None,
            listen_stats,
            instance_id: 0,
        }
    }

    fn pre_start(&self) {
        info!(
            "started {} SRT[{}_v{}#{}]",
            self.server_type,
            self.server.name(),
            self.server_version,
            self.instance_id,
        );
        self.listen_stats.add_running_runtime();
    }

    fn pre_stop(&self) {
        info!(
            "stopping {} SRT[{}_v{}#{}]",
            self.server_type,
            self.server.name(),
            self.server_version,
            self.instance_id,
        );
    }

    fn post_stop(&self) {
        info!(
            "stopped {} SRT[{}_v{}#{}]",
            self.server_type,
            self.server.name(),
            self.server_version,
            self.instance_id,
        );
        self.listen_stats.del_running_runtime();
    }

    async fn run(
        mut self,
        mut listener: LimitedTcpListener,
        mut server_reload_channel: broadcast::Receiver<ServerReloadCommand>,
    ) {
        use broadcast::error::RecvError;

        loop {
            tokio::select! {
                biased;

                ev = server_reload_channel.recv() => {
                   match ev {
                        Ok(ServerReloadCommand::ReloadVersion(version)) => {
                            info!("SRT[{}_v{}#{}] received reload request from v{version}",
                                self.server.name(), self.server_version, self.instance_id);
                            let new_server = self.server.get_reloaded();
                            self.server_version = new_server.version();
                            self.server = new_server;
                            continue;
                        }
                        Ok(ServerReloadCommand::QuitRuntime) => {},
                        Err(RecvError::Closed) => {},
                        Err(RecvError::Lagged(dropped)) => {
                            warn!("SRT[{}_v{}#{}] server {} reload notify channel overflowed, {dropped} msg dropped",
                                self.server.name(), self.server_version, self.instance_id, self.server.name());
                            continue;
                        },
                    }

                    info!("SRT[{}_v{}#{}] will go offline",
                        self.server.name(), self.server_version, self.instance_id);
                    self.pre_stop();
                    let accept_again = listener.set_offline();
                    if accept_again {
                        info!("SRT[{}_v{}#{}] will accept all pending connections",
                            self.server.name(), self.server_version, self.instance_id);
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
                                    native_socket_addr(peer_addr),
                                    native_socket_addr(local_addr),
                                );
                                Ok(())
                            }
                            Ok(None) => {
                                info!("SRT[{}_v{}#{}] offline",
                                    self.server.name(), self.server_version, self.instance_id);
                                Err(())
                            }
                            Err(e) => {
                                self.listen_stats.add_failed();
                                warn!("SRT[{}_v{}#{}] accept: {e:?}",
                                    self.server.name(), self.server_version, self.instance_id);
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
        let server = self.server.clone();

        let mut cc_info = ClientConnectionInfo::new(peer_addr, local_addr);
        cc_info.set_tcp_raw_fd(stream.as_raw_fd());
        if let Some(worker_id) = self.worker_id {
            cc_info.set_worker_id(Some(worker_id));
            tokio::spawn(async move {
                server.run_tcp_task(stream, cc_info).await;
            });
        } else if let Some(rt) = crate::runtime::worker::select_handle() {
            cc_info.set_worker_id(Some(rt.id));
            rt.handle.spawn(async move {
                server.run_tcp_task(stream, cc_info).await;
            });
        } else {
            tokio::spawn(async move {
                server.run_tcp_task(stream, cc_info).await;
            });
        }
    }

    fn get_rt_handle(&mut self, listen_in_worker: bool) -> Handle {
        if listen_in_worker {
            if let Some(rt) = crate::runtime::worker::select_listen_handle() {
                self.worker_id = Some(rt.id);
                return rt.handle;
            }
        }
        Handle::current()
    }

    fn into_running(
        mut self,
        listener: std::net::TcpListener,
        listen_in_worker: bool,
        server_reload_channel: broadcast::Receiver<ServerReloadCommand>,
    ) {
        let handle = self.get_rt_handle(listen_in_worker);
        handle.spawn(async move {
            // make sure the listen socket associated with the correct reactor
            match tokio::net::TcpListener::from_std(listener) {
                Ok(listener) => {
                    self.pre_start();
                    self.run(LimitedTcpListener::new(listener), server_reload_channel)
                        .await;
                }
                Err(e) => {
                    warn!(
                        "SRT[{}_v{}#{}] listen async: {e:?}",
                        self.server.name(),
                        self.server_version,
                        self.instance_id
                    );
                }
            }
        });
    }

    pub fn run_all_instances(
        &self,
        listen_config: &TcpListenConfig,
        listen_in_worker: bool,
        server_reload_sender: &broadcast::Sender<ServerReloadCommand>,
    ) -> anyhow::Result<()> {
        let mut instance_count = listen_config.instance();
        if listen_in_worker {
            let worker_count = crate::runtime::worker::worker_count();
            if worker_count > 0 {
                instance_count = worker_count;
            }
        }

        for i in 0..instance_count {
            let mut runtime = self.clone();
            runtime.instance_id = i;

            let listener = g3_socket::tcp::new_std_listener(listen_config)?;
            runtime.into_running(listener, listen_in_worker, server_reload_sender.subscribe());
        }
        Ok(())
    }
}
