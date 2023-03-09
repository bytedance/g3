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
use tokio::runtime::Handle;
use tokio::sync::{broadcast, watch};

use g3_daemon::listen::ListenStats;
use g3_io_ext::LimitedTcpListener;
use g3_socket::util::native_socket_addr;
use g3_types::net::TcpListenConfig;

use crate::config::server::ServerConfig;
use crate::serve::{ArcServer, ServerReloadCommand, ServerRunContext};

pub(crate) trait AuxiliaryServerConfig {
    fn next_server(&self) -> &str;
    fn run_tcp_task(
        &self,
        rt_handle: Handle,
        next_server: ArcServer,
        stream: TcpStream,
        peer_addr: SocketAddr,
        local_addr: SocketAddr,
        ctx: ServerRunContext,
    );
}

#[derive(Clone)]
pub(crate) struct AuxiliaryTcpPortRuntime {
    server: ArcServer,
    server_type: &'static str,
    server_version: usize,
    worker_id: Option<usize>,
    listen_stats: Arc<ListenStats>,
    instance_id: usize,
}

impl AuxiliaryTcpPortRuntime {
    pub(crate) fn new<C: ServerConfig>(server: &ArcServer, server_config: &C) -> Self {
        AuxiliaryTcpPortRuntime {
            server: Arc::clone(server),
            server_type: server_config.server_type(),
            server_version: server.version(),
            worker_id: None,
            listen_stats: server.get_listen_stats(),
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

    fn rt_handle(&self) -> (Handle, Option<usize>) {
        if let Some(id) = self.worker_id {
            (Handle::current(), Some(id))
        } else if let Some(rt) = g3_daemon::runtime::worker::select_handle() {
            (rt.handle, Some(rt.id))
        } else {
            (Handle::current(), None)
        }
    }

    async fn run<C>(
        self,
        mut listener: LimitedTcpListener,
        mut cfg_receiver: watch::Receiver<Option<C>>,
    ) where
        C: AuxiliaryServerConfig + Clone,
    {
        use broadcast::error::RecvError;

        let mut aux_config = match cfg_receiver.borrow().clone() {
            Some(c) => c,
            None => return,
        };
        let mut next_server_name = aux_config.next_server().to_string();
        let (mut next_server, mut next_server_reload_channel) =
            crate::serve::get_with_notifier(&next_server_name);
        let run_ctx = ServerRunContext::new();

        loop {
            let mut reload_next_server = false;

            tokio::select! {
                biased;

                // use update in place channel instead of common server reload channel for local config reload
                ev = cfg_receiver.changed() => {
                    if ev.is_err() {
                        warn!("SRT[{}_v{}#{}] quit as cfg channel closed",
                            self.server.name(), self.server_version, self.instance_id);
                        break;
                    }
                    let value = cfg_receiver.borrow().clone();
                    match value {
                        None => {
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
                        Some(config) => {
                            aux_config = config;
                            if aux_config.next_server().ne(&next_server_name) {
                                info!("SRT[{}_v{}#{}] will use next server '{}' instead of '{next_server_name}'",
                                    self.server.name(), self.server_version, self.instance_id, aux_config.next_server());
                                next_server_name = aux_config.next_server().to_string();
                                reload_next_server = true;
                            }
                        }
                    }
                }
                ev = next_server_reload_channel.recv() => {
                    match ev {
                        Ok(ServerReloadCommand::ReloadVersion(version)) => {
                            info!("SRT[{}_v{}#{}] reload next server {next_server_name} to v{version}",
                                self.server.name(), self.server_version, self.instance_id);
                            reload_next_server = true;
                        }
                        Ok(ServerReloadCommand::QuitRuntime) | Err(RecvError::Closed) => {
                            info!("SRT[{}_v{}#{}] next server {next_server_name} quit, reload it",
                                self.server.name(), self.server_version, self.instance_id);
                            reload_next_server = true;
                        }
                        Err(RecvError::Lagged(dropped)) => {
                            warn!("SRT[{}_v{}#{}] next server {next_server_name} reload notify channel overflowed, {dropped} msg dropped",
                                self.server.name(), self.server_version, self.instance_id);
                            continue
                        },
                    }
                }
                result = listener.accept() => {
                    match result {
                        Ok(Some((stream, peer_addr, local_addr))) => {
                            self.listen_stats.add_accepted();
                            let (rt_handle, worker_id) = self.rt_handle();
                            let mut run_ctx = run_ctx.clone();
                            run_ctx.worker_id = worker_id;
                            aux_config.run_tcp_task(
                                rt_handle,
                                next_server.clone(),
                                stream,
                                native_socket_addr(peer_addr),
                                native_socket_addr(local_addr),
                                run_ctx,
                            );
                        }
                        Ok(None) => {
                            info!("SRT[{}_v{}#{}] offline",
                                self.server.name(), self.server_version, self.instance_id);
                            break;
                        }
                        Err(e) => {
                            self.listen_stats.add_failed();
                            warn!("SRT[{}_v{}#{}] accept: {e:?}",
                                self.server.name(), self.server_version, self.instance_id);
                        }
                    }
                }
            }

            if reload_next_server {
                let result = crate::serve::get_with_notifier(&next_server_name);
                next_server = result.0;
                next_server_reload_channel = result.1;
            }
        }
        self.post_stop();
    }

    fn get_rt_handle(&mut self, listen_in_worker: bool) -> Handle {
        if listen_in_worker {
            if let Some(rt) = g3_daemon::runtime::worker::select_listen_handle() {
                self.worker_id = Some(rt.id);
                return rt.handle;
            }
        }
        Handle::current()
    }

    fn into_running<C>(
        mut self,
        listener: std::net::TcpListener,
        listen_in_worker: bool,
        cfg_receiver: watch::Receiver<Option<C>>,
    ) where
        C: AuxiliaryServerConfig + Clone + Send + Sync + 'static,
    {
        let handle = self.get_rt_handle(listen_in_worker);
        handle.spawn(async move {
            // make sure the listen socket associated with the correct reactor
            match tokio::net::TcpListener::from_std(listener) {
                Ok(listener) => {
                    self.pre_start();
                    self.run(LimitedTcpListener::new(listener), cfg_receiver)
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

    pub(crate) fn run_all_instances<C>(
        &self,
        listen_config: &TcpListenConfig,
        listen_in_worker: bool,
        cfg_receiver: &watch::Sender<Option<C>>,
    ) -> anyhow::Result<()>
    where
        C: AuxiliaryServerConfig + Clone + Send + Sync + 'static,
    {
        let mut instance_count = listen_config.instance();
        if listen_in_worker {
            let worker_count = g3_daemon::runtime::worker::worker_count();
            if worker_count > 0 {
                instance_count = worker_count;
            }
        }

        for i in 0..instance_count {
            let mut runtime = self.clone();
            runtime.instance_id = i;

            let listener = g3_socket::tcp::new_std_listener(listen_config)?;
            runtime.into_running(listener, listen_in_worker, cfg_receiver.subscribe());
        }
        Ok(())
    }
}
