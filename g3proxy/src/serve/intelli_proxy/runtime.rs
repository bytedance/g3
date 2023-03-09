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
use tokio::sync::{broadcast, mpsc, watch};

use g3_io_ext::LimitedTcpListener;
use g3_socket::util::native_socket_addr;
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::limit::{GaugeSemaphore, GaugeSemaphorePermit};

use super::{detect_tcp_proxy_protocol, DetectedProxyProtocol};
use crate::config::server::intelli_proxy::IntelliProxyConfig;
use crate::config::server::ServerConfig;
use crate::serve::{ArcServer, ListenStats, ServerReloadCommand, ServerRunContext};

struct DetectedTcpStream {
    stream: TcpStream,
    peer_addr: SocketAddr,
    local_addr: SocketAddr,
    protocol: DetectedProxyProtocol,
}

#[derive(Clone)]
pub(crate) struct IntelliProxyRuntime {
    config: IntelliProxyConfig,
    server_version: usize,
    // keep a ref to arc server to make sure the cfg channel won't close
    #[allow(unused)]
    ref_server: ArcServer,
    worker_id: Option<usize>,
    listen_stats: Arc<ListenStats>,

    cfg_receiver: watch::Receiver<Option<IntelliProxyConfig>>,
    instance_id: usize,

    ingress_net_filter: Option<Arc<AclNetworkRule>>,
}

impl IntelliProxyRuntime {
    pub(crate) fn new(
        config: IntelliProxyConfig,
        cfg_receiver: watch::Receiver<Option<IntelliProxyConfig>>,
        server: &ArcServer,
    ) -> Self {
        let ingress_net_filter = config
            .ingress_net_filter
            .as_ref()
            .map(|builder| Arc::new(builder.build()));

        IntelliProxyRuntime {
            config,
            server_version: server.version(),
            ref_server: Arc::clone(server),
            worker_id: None,
            listen_stats: server.get_listen_stats(),
            cfg_receiver,
            instance_id: 0,
            ingress_net_filter,
        }
    }

    fn pre_start(&self) {
        info!(
            "started {} SRT[{}_v{}#{}]",
            self.config.server_type(),
            self.config.name(),
            self.server_version,
            self.instance_id,
        );
        self.listen_stats.add_running_runtime();
    }

    fn post_stop(&self) {
        info!(
            "stopped {} SRT[{}_v{}#{}]",
            self.config.server_type(),
            self.config.name(),
            self.server_version,
            self.instance_id,
        );
        self.listen_stats.del_running_runtime();
    }

    async fn run(mut self, mut stream_receiver: mpsc::Receiver<DetectedTcpStream>) {
        use broadcast::error::RecvError;

        let (mut http_next_server, mut http_next_server_reload_channel) =
            crate::serve::get_with_notifier(&self.config.http_server);
        let mut http_run_ctx = ServerRunContext::new(
            &http_next_server.escaper(),
            &http_next_server.user_group(),
            &http_next_server.auditor(),
        );

        let (mut socks_next_server, mut socks_next_server_reload_channel) =
            crate::serve::get_with_notifier(&self.config.socks_server);
        let mut socks_run_ctx = ServerRunContext::new(
            &socks_next_server.escaper(),
            &socks_next_server.user_group(),
            &socks_next_server.auditor(),
        );

        loop {
            let mut reload_http_server = false;
            let mut reload_socks_server = false;

            tokio::select! {
                biased;

                // use update in place channel instead of common server reload channel for local config reload
                ev = self.cfg_receiver.changed() => {
                    if ev.is_err() {
                        // we have keep a ref to the server to make sure that cfg_receiver won't close
                        warn!("SRT[{}_v{}#{}] quit as cfg update channel closed",
                            self.config.name(), self.server_version, self.instance_id);
                    } else {
                        let old_http_server = self.config.http_server.clone();
                        let old_socks_server = self.config.socks_server.clone();
                        self.config = match self.cfg_receiver.borrow().clone() {
                            Some(config) => config,
                            None => continue, // server aborted, wait all connections spawned
                        };

                        self.ingress_net_filter = self
                            .config
                            .ingress_net_filter
                            .as_ref()
                            .map(|builder| Arc::new(builder.build()));

                        if self.config.http_server.ne(&old_http_server) {
                            info!("SRT[{}_v{}#{}] will use next http server '{}' instead of '{old_http_server}'",
                                self.config.name(), self.server_version, self.instance_id, self.config.http_server);
                            reload_http_server = true;
                        }

                        if self.config.socks_server.ne(&old_socks_server) {
                            info!("SRT[{}_v{}#{}] will use next socks server '{}' instead of '{old_socks_server}'",
                                self.config.name(), self.server_version, self.instance_id, self.config.socks_server);
                            reload_socks_server = true;
                        }
                    }
                }
                ev = http_next_server_reload_channel.recv() => {
                    match ev {
                        Ok(ServerReloadCommand::ReloadVersion(version)) => {
                            info!("SRT[{}_v{}#{}] reload next http server {} to v{}",
                                self.config.name(), self.server_version, self.instance_id, self.config.http_server, version);
                            reload_http_server = true;
                        }
                        Ok(ServerReloadCommand::ReloadEscaper) => {
                            let escaper_name = http_next_server.escaper();
                            info!("SRT[{}_v{}#{}] will reload http escaper {escaper_name}",
                                self.config.name(), self.server_version, self.instance_id);
                            http_run_ctx.update_escaper(&escaper_name);
                        },
                        Ok(ServerReloadCommand::ReloadUserGroup) => {
                            let user_group_name = http_next_server.user_group();
                            info!("SRT[{}_v{}#{}] will reload http user group {user_group_name}",
                                self.config.name(), self.server_version, self.instance_id);
                            http_run_ctx.update_user_group(&user_group_name);
                        },
                        Ok(ServerReloadCommand::ReloadAuditor) => {
                            let auditor_name = http_next_server.auditor();
                            info!("SRT[{}_v{}#{}] will reload http auditor {auditor_name}",
                                self.config.name(), self.server_version, self.instance_id);
                            http_run_ctx.update_audit_handle(&auditor_name);
                        },
                        Ok(ServerReloadCommand::QuitRuntime) | Err(RecvError::Closed) => {
                            info!("SRT[{}_v{}#{}] next http server {} quit, reload it",
                                self.config.name(), self.server_version, self.instance_id, self.config.http_server);
                            reload_http_server = true;
                        }
                        Err(RecvError::Lagged(dropped)) => {
                            warn!("SRT[{}_v{}#{}] next http server {} reload notify channel overflowed, {dropped} msg dropped",
                                self.config.name(), self.server_version, self.instance_id, http_next_server.name());
                            continue
                        },
                    }
                }
                ev = socks_next_server_reload_channel.recv() => {
                    match ev {
                        Ok(ServerReloadCommand::ReloadVersion(version)) => {
                            info!("SRT[{}_v{}#{}] reload next socks server {} to v{}",
                                self.config.name(), self.server_version, self.instance_id, self.config.socks_server, version);
                            reload_socks_server = true;
                        }
                        Ok(ServerReloadCommand::ReloadEscaper) => {
                            let escaper_name = socks_next_server.escaper();
                            info!("SRT[{}_v{}#{}] will reload socks escaper {escaper_name}",
                                self.config.name(), self.server_version, self.instance_id);
                            socks_run_ctx.update_escaper(&escaper_name);
                        },
                        Ok(ServerReloadCommand::ReloadUserGroup) => {
                            let user_group_name = socks_next_server.user_group();
                            info!("SRT[{}_v{}#{}] will reload socks user group {user_group_name}",
                                self.config.name(), self.server_version, self.instance_id);
                            socks_run_ctx.update_user_group(&user_group_name);
                        },
                        Ok(ServerReloadCommand::ReloadAuditor) => {
                            let auditor_name = socks_next_server.auditor();
                            info!("SRT[{}_v{}#{}] will reload socks auditor {auditor_name}",
                                self.config.name(), self.server_version, self.instance_id);
                            socks_run_ctx.update_audit_handle(&auditor_name);
                        },
                        Ok(ServerReloadCommand::QuitRuntime) | Err(RecvError::Closed) => {
                            info!("SRT[{}_v{}#{}] next socks server {} quit, reload it",
                                self.config.name(), self.server_version, self.instance_id, self.config.socks_server);
                            reload_socks_server = true;
                        }
                        Err(RecvError::Lagged(dropped)) => {
                            warn!("SRT[{}_v{}#{}] next socks server {} reload notify channel overflowed, {dropped} msg dropped",
                                self.config.name(), self.server_version, self.instance_id, socks_next_server.name());
                            continue
                        },
                    }
                }
                data = stream_receiver.recv() => {
                    match data {
                        Some(d) => {
                            self.listen_stats.add_accepted();
                            match d.protocol {
                                DetectedProxyProtocol::Http => {
                                    self.run_task(
                                        d,
                                        http_next_server.clone(),
                                        http_run_ctx.clone(),
                                    );
                                }
                                DetectedProxyProtocol::Socks => {
                                    self.run_task(
                                        d,
                                        socks_next_server.clone(),
                                        socks_run_ctx.clone(),
                                    );
                                }
                                _ => {}
                            }
                        }
                        None => {
                            info!("SRT[{}_v{}#{}] quit after all connections handled",
                                self.config.name(), self.server_version, self.instance_id);
                            break;
                        },
                    }
                }
            }

            if reload_http_server {
                let result = crate::serve::get_with_notifier(&self.config.http_server);
                http_next_server = result.0;
                http_next_server_reload_channel = result.1;

                // if escaper changed, reload it
                let old_escaper = http_run_ctx.current_escaper();
                let new_escaper = http_next_server.escaper();
                if old_escaper.ne(&new_escaper) {
                    info!("SRT[{}_v{}#{}] will use http escaper '{new_escaper}' instead of '{old_escaper}'",
                                    self.config.name(), self.server_version, self.instance_id);
                    http_run_ctx.update_escaper(&new_escaper);
                }

                // if user group changed, reload it
                let old_user_group = http_run_ctx.current_user_group();
                let new_user_group = http_next_server.user_group();
                if old_user_group.ne(&new_user_group) {
                    info!("SRT[{}_v{}#{}] will use http user group '{new_user_group}' instead of '{old_user_group}'",
                                    self.config.name(), self.server_version, self.instance_id);
                    http_run_ctx.update_user_group(&new_user_group);
                }

                // if auditor changed, reload it
                let old_auditor = http_run_ctx.current_auditor();
                let new_auditor = http_next_server.auditor();
                if old_auditor.ne(&new_auditor) {
                    info!("SRT[{}_v{}#{}] will use http auditor '{new_auditor}' instead of '{old_auditor}'",
                                    self.config.name(), self.server_version, self.instance_id);
                    http_run_ctx.update_audit_handle(&new_auditor);
                }
            }

            if reload_socks_server {
                let result = crate::serve::get_with_notifier(&self.config.socks_server);
                socks_next_server = result.0;
                socks_next_server_reload_channel = result.1;

                // if escaper changed, reload it
                let old_escaper = socks_run_ctx.current_escaper();
                let new_escaper = socks_next_server.escaper();
                if old_escaper.ne(&new_escaper) {
                    info!("SRT[{}_v{}#{}] will use socks escaper '{new_escaper}' instead of '{old_escaper}'",
                                    self.config.name(), self.server_version, self.instance_id);
                    socks_run_ctx.update_escaper(&new_escaper);
                }

                // if user group changed, reload it
                let old_user_group = socks_run_ctx.current_user_group();
                let new_user_group = socks_next_server.user_group();
                if old_user_group.ne(&new_user_group) {
                    info!("SRT[{}_v{}#{}] will use socks user group '{new_user_group}' instead of '{old_user_group}'",
                                    self.config.name(), self.server_version, self.instance_id);
                    socks_run_ctx.update_user_group(&new_user_group);
                }

                // if auditor changed, reload it
                let old_auditor = socks_run_ctx.current_auditor();
                let new_auditor = socks_next_server.auditor();
                if old_auditor.ne(&new_auditor) {
                    info!("SRT[{}_v{}#{}] will use socks auditor '{new_auditor}' instead of '{old_auditor}'",
                                    self.config.name(), self.server_version, self.instance_id);
                    socks_run_ctx.update_audit_handle(&new_auditor);
                }
            }
        }
        self.post_stop();
    }

    fn run_task(&self, d: DetectedTcpStream, server: ArcServer, mut ctx: ServerRunContext) {
        let ingress_net_filter = self.ingress_net_filter.clone();
        let listen_stats = self.listen_stats.clone();

        let rt_handle = if let Some(worker_id) = self.worker_id {
            ctx.worker_id = Some(worker_id);
            tokio::runtime::Handle::current()
        } else if let Some(rt) = g3_daemon::runtime::worker::select_handle() {
            ctx.worker_id = Some(rt.id);
            rt.handle
        } else {
            tokio::runtime::Handle::current()
        };

        rt_handle.spawn(async move {
            if let Some(filter) = ingress_net_filter {
                let (_, action) = filter.check(d.peer_addr.ip());
                match action {
                    AclAction::Permit | AclAction::PermitAndLog => {}
                    AclAction::Forbid | AclAction::ForbidAndLog => {
                        listen_stats.add_dropped();
                        return;
                    }
                }
            }

            server
                .run_tcp_task(d.stream, d.peer_addr, d.local_addr, ctx)
                .await
        });
    }

    fn get_rt_handle(&mut self) -> Handle {
        if self.config.listen_in_worker {
            if let Some(rt) = g3_daemon::runtime::worker::select_listen_handle() {
                self.worker_id = Some(rt.id);
                return rt.handle;
            }
        }
        Handle::current()
    }

    fn into_running(mut self, listener: std::net::TcpListener) {
        let (stream_sender, stream_receiver) =
            mpsc::channel::<DetectedTcpStream>(self.config.protocol_detection_channel_size);

        let handle = self.get_rt_handle();
        handle.spawn(async move {
            // make sure the listen socket associated with the correct reactor
            match tokio::net::TcpListener::from_std(listener) {
                Ok(listener) => {
                    let listen = IntelliProxyListen {
                        config: self.config.clone(),
                        server_version: self.server_version,
                        cfg_receiver: self.cfg_receiver.clone(),
                        listen_stats: Arc::clone(&self.listen_stats),
                        listener: LimitedTcpListener::new(listener),
                        stream_sender,
                        instance_id: self.instance_id,
                    };
                    self.pre_start();
                    tokio::spawn(async move {
                        self.run(stream_receiver).await;
                    });
                    listen.run().await;
                }
                Err(e) => {
                    warn!(
                        "SRT[{}_v{}#{}] listen async: {e:?}",
                        self.config.name(),
                        self.server_version,
                        self.instance_id
                    );
                }
            }
        });
    }

    pub(super) fn run_all_instances(&self) -> anyhow::Result<()> {
        let mut instance_count = self.config.listen.instance();
        if self.config.listen_in_worker {
            let worker_count = g3_daemon::runtime::worker::worker_count();
            if worker_count > 0 {
                instance_count = worker_count;
            }
        }

        for i in 0..instance_count {
            let mut runtime = self.clone();
            runtime.instance_id = i;

            let listener = g3_socket::tcp::new_std_listener(&self.config.listen)?;
            runtime.into_running(listener);
        }
        Ok(())
    }
}

pub(crate) struct IntelliProxyListen {
    config: IntelliProxyConfig,
    server_version: usize,
    cfg_receiver: watch::Receiver<Option<IntelliProxyConfig>>,
    listen_stats: Arc<ListenStats>,
    listener: LimitedTcpListener,
    stream_sender: mpsc::Sender<DetectedTcpStream>,
    instance_id: usize,
}

impl IntelliProxyListen {
    fn pre_stop(&self) {
        info!(
            "stopping {} SRT[{}_v{}#{}]",
            self.config.server_type(),
            self.config.name(),
            self.server_version,
            self.instance_id,
        );
    }

    async fn run(mut self) {
        let mut spawn_sema = GaugeSemaphore::new(self.config.protocol_detection_max_jobs);

        loop {
            tokio::select! {
                biased;

                // use update in place channel instead of common server reload channel for local config reload
                ev = self.cfg_receiver.changed() => {
                    if ev.is_err() {
                        // we have keep a ref to the server to make sure that cfg_receiver won't close
                        warn!("SRT[{}_v{}#{}] stop listen as cfg update channel closed",
                            self.config.name(), self.server_version, self.instance_id);
                        break;
                    } else {
                        match self.cfg_receiver.borrow().clone() {
                            Some(config) => {
                                spawn_sema = spawn_sema.new_updated(config.protocol_detection_max_jobs);
                                self.config = config;
                            }
                            None => {
                                info!("SRT[{}_v{}#{}] will go offline",
                                self.config.name(), self.server_version, self.instance_id);
                                self.pre_stop();
                                let accept_again = self.listener.set_offline();
                                if accept_again {
                                    info!("SRT[{}_v{}#{}] will accept all pending connections",
                                        self.config.name(), self.server_version, self.instance_id);
                                    continue;
                                } else {
                                    break;
                                }
                            }
                        }
                    }
                }
                result = self.listener.accept() => {
                    match result {
                        Ok(Some((stream, peer_addr, local_addr))) => {
                            if let Ok(permit) = spawn_sema.try_acquire() {
                                self.send_to_detection(
                                    stream,
                                    native_socket_addr(peer_addr),
                                    native_socket_addr(local_addr),
                                    permit,
                                );
                            } else {
                                // limit reached
                                self.listen_stats.add_failed();
                            }
                        }
                        Ok(None) => {
                            info!("SRT[{}_v{}#{}] offline",
                                self.config.name(), self.server_version, self.instance_id);
                            break;
                        }
                        Err(e) => {
                            self.listen_stats.add_failed();
                            warn!("SRT[{}_v{}#{}] accept: {:?}",
                                self.config.name(), self.server_version, self.instance_id, e);
                        }
                    }
                }
            }
        }
    }

    fn send_to_detection(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
        local_addr: SocketAddr,
        permit: GaugeSemaphorePermit,
    ) {
        let listen_stats = Arc::clone(&self.listen_stats);
        let stream_sender = self.stream_sender.clone();
        let detection_timeout = self.config.protocol_detection_timeout;
        tokio::spawn(async move {
            match tokio::time::timeout(detection_timeout, detect_tcp_proxy_protocol(&stream)).await
            {
                Ok(Ok(protocol)) => {
                    if matches!(protocol, DetectedProxyProtocol::Unknown) {
                        // unknown protocol
                        listen_stats.add_failed();
                        return;
                    }

                    let d = DetectedTcpStream {
                        stream,
                        peer_addr,
                        local_addr,
                        protocol,
                    };
                    if stream_sender.send(d).await.is_err() {
                        // send failed
                        listen_stats.add_failed();
                    }
                }
                Ok(Err(_)) => {
                    // io error
                    listen_stats.add_failed();
                }
                Err(_) => {
                    // timed out
                    listen_stats.add_failed();
                }
            }
        });
        drop(permit); // make sure permit is moved in
    }
}
