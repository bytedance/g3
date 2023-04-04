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
use tokio::sync::broadcast;

use g3_daemon::listen::ListenStats;
use g3_io_ext::LimitedTcpListener;
use g3_socket::util::native_socket_addr;
use g3_types::net::TcpListenConfig;

use crate::config::server::ServerConfig;
use crate::serve::{ArcServer, ServerReloadCommand, ServerRunContext};

#[derive(Clone)]
pub(crate) struct OrdinaryTcpServerRuntime {
    server: ArcServer,
    server_type: &'static str,
    server_version: usize,
    worker_id: Option<usize>,
    listen_stats: Arc<ListenStats>,
    instance_id: usize,
}

impl OrdinaryTcpServerRuntime {
    pub(crate) fn new<C: ServerConfig>(server: &ArcServer, server_config: &C) -> Self {
        OrdinaryTcpServerRuntime {
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

    async fn run(
        mut self,
        mut listener: LimitedTcpListener,
        mut server_reload_channel: broadcast::Receiver<ServerReloadCommand>,
    ) {
        use broadcast::error::RecvError;

        let mut run_ctx = ServerRunContext::new(
            self.server.escaper(),
            self.server.user_group(),
            self.server.auditor(),
        );

        loop {
            tokio::select! {
                biased;

                ev = server_reload_channel.recv() => {
                    let cmd = match ev {
                        Ok(ServerReloadCommand::ReloadVersion(version)) => {
                            info!("SRT[{}_v{}#{}] received reload request from v{version}",
                                self.server.name(), self.server_version, self.instance_id);
                            match crate::serve::get_server(self.server.name()) {
                                Ok(server) => {
                                    self.server_version = server.version();
                                    self.server = server;
                                    ServerReloadCommand::ReloadVersion(version)
                                }
                                Err(_) => {
                                    info!("SRT[{}_v{}#{}] will quit as no server v{version}+ found",
                                        self.server.name(), self.server_version, self.instance_id);
                                    ServerReloadCommand::QuitRuntime
                                }
                            }
                        }
                        Ok(ServerReloadCommand::ReloadEscaper) => ServerReloadCommand::ReloadEscaper,
                        Ok(ServerReloadCommand::ReloadUserGroup) => ServerReloadCommand::ReloadUserGroup,
                        Ok(ServerReloadCommand::ReloadAuditor) => ServerReloadCommand::ReloadAuditor,
                        Ok(ServerReloadCommand::QuitRuntime) => ServerReloadCommand::QuitRuntime,
                        Err(RecvError::Closed) => ServerReloadCommand::QuitRuntime,
                        Err(RecvError::Lagged(dropped)) => {
                            warn!("SRT[{}_v{}#{}] server {} reload notify channel overflowed, {dropped} msg dropped",
                                self.server.name(), self.server_version, self.instance_id, self.server.name());
                            continue
                        },
                    };
                    match cmd {
                        ServerReloadCommand::ReloadVersion(_) => {
                            // if escaper changed, reload it
                            let old_escaper = run_ctx.current_escaper();
                            let new_escaper = self.server.escaper();
                            if old_escaper.ne(new_escaper) {
                                info!("SRT[{}_v{}#{}] will use escaper '{new_escaper}' instead of '{old_escaper}'",
                                    self.server.name(), self.server_version, self.instance_id);
                                run_ctx.update_escaper(new_escaper);
                            }

                            // if user group changed, reload it
                            let old_user_group = run_ctx.current_user_group();
                            let new_user_group = self.server.user_group();
                            if old_user_group.ne(new_user_group) {
                                info!("SRT[{}_v{}#{}] will use user group '{new_user_group}' instead of '{old_user_group}'",
                                    self.server.name(), self.server_version, self.instance_id);
                                run_ctx.update_user_group(new_user_group);
                            }

                            // if auditor changed, reload it
                            let old_auditor = run_ctx.current_auditor();
                            let new_auditor = self.server.auditor();
                            if old_auditor.ne(new_auditor) {
                                info!("SRT[{}_v{}#{}] will use auditor '{new_auditor}' instead of '{old_auditor}'",
                                    self.server.name(), self.server_version, self.instance_id);
                                run_ctx.update_audit_handle(new_auditor);
                            }
                        }
                        ServerReloadCommand::ReloadEscaper => {
                            let escaper_name = self.server.escaper();
                            info!("SRT[{}_v{}#{}] will reload escaper {escaper_name}",
                                self.server.name(), self.server_version, self.instance_id);
                            run_ctx.update_escaper(escaper_name);
                        }
                        ServerReloadCommand::ReloadUserGroup => {
                            let user_group_name = self.server.user_group();
                            info!("SRT[{}_v{}#{}] will reload user group {user_group_name}",
                                self.server.name(), self.server_version, self.instance_id);
                            run_ctx.update_user_group(user_group_name);
                        }
                        ServerReloadCommand::ReloadAuditor => {
                            let auditor_name = self.server.auditor();
                            info!("SRT[{}_v{}#{}] will reload auditor {auditor_name}",
                                self.server.name(), self.server_version, self.instance_id);
                            run_ctx.update_audit_handle(auditor_name);
                        }
                        ServerReloadCommand::QuitRuntime => {
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
                    }
                }
                result = listener.accept() => {
                    if listener.accept_current_available(result, &|result| {
                        match result {
                            Ok(Some((stream, peer_addr, local_addr))) => {
                                self.listen_stats.add_accepted();
                                self.run_task(
                                    stream,
                                    native_socket_addr(peer_addr),
                                    native_socket_addr(local_addr),
                                    run_ctx.clone(),
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

    fn run_task(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
        local_addr: SocketAddr,
        mut run_ctx: ServerRunContext,
    ) {
        let server = Arc::clone(&self.server);

        if let Some(worker_id) = self.worker_id {
            run_ctx.worker_id = Some(worker_id);
            tokio::spawn(async move {
                server
                    .run_tcp_task(stream, peer_addr, local_addr, run_ctx)
                    .await;
            });
        } else if let Some(rt) = g3_daemon::runtime::worker::select_handle() {
            run_ctx.worker_id = Some(rt.id);
            rt.handle.spawn(async move {
                server
                    .run_tcp_task(stream, peer_addr, local_addr, run_ctx)
                    .await;
            });
        } else {
            tokio::spawn(async move {
                server
                    .run_tcp_task(stream, peer_addr, local_addr, run_ctx)
                    .await;
            });
        }
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

    pub(crate) fn run_all_instances(
        &self,
        listen_config: &TcpListenConfig,
        listen_in_worker: bool,
        server_reload_sender: &broadcast::Sender<ServerReloadCommand>,
    ) -> anyhow::Result<()> {
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
            runtime.into_running(listener, listen_in_worker, server_reload_sender.subscribe());
        }
        Ok(())
    }
}
