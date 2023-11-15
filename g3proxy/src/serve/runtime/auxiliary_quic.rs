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

use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::os::fd::{AsRawFd, RawFd};
use std::sync::Arc;

use log::{info, warn};
use quinn::Endpoint;
use tokio::runtime::Handle;
use tokio::sync::{broadcast, watch};

use g3_daemon::listen::ListenStats;
use g3_daemon::server::ClientConnectionInfo;
use g3_socket::util::native_socket_addr;
use g3_types::net::UdpListenConfig;

use super::AuxiliaryServerConfig;
use crate::config::server::ServerConfig;
use crate::serve::{ArcServer, ServerReloadCommand, ServerRunContext};

#[derive(Clone)]
pub(crate) struct AuxiliaryQuicPortRuntime {
    server: ArcServer,
    server_type: &'static str,
    server_version: usize,
    worker_id: Option<usize>,
    listen_config: UdpListenConfig,
    rebind_port: Option<u16>,
    listen_stats: Arc<ListenStats>,
    instance_id: usize,
}

impl AuxiliaryQuicPortRuntime {
    pub(crate) fn new<C: ServerConfig>(
        server: &ArcServer,
        server_config: &C,
        listen_config: UdpListenConfig,
        rebind_port: Option<u16>,
    ) -> Self {
        AuxiliaryQuicPortRuntime {
            server: Arc::clone(server),
            server_type: server_config.server_type(),
            server_version: server.version(),
            worker_id: None,
            listen_config,
            rebind_port,
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
        if let Some(worker_id) = self.worker_id {
            (Handle::current(), Some(worker_id))
        } else if let Some(rt) = g3_daemon::runtime::worker::select_handle() {
            (rt.handle, Some(rt.id))
        } else {
            (Handle::current(), None)
        }
    }

    async fn run<C>(
        mut self,
        listener: Endpoint,
        mut listen_addr: SocketAddr,
        mut sock_raw_fd: RawFd,
        mut cfg_receiver: watch::Receiver<Option<C>>,
    ) where
        C: AuxiliaryServerConfig + Send + Clone + 'static,
    {
        use broadcast::error::RecvError;

        let mut aux_config = match cfg_receiver.borrow().clone() {
            Some(c) => c,
            None => return,
        };
        let mut next_server_name = aux_config.next_server().clone();
        let (mut next_server, mut next_server_reload_channel) =
            crate::serve::get_with_notifier(&next_server_name);
        let mut run_ctx = ServerRunContext::new(
            next_server.escaper(),
            next_server.user_group(),
            next_server.auditor(),
        );

        loop {
            let mut reload_next_server = false;

            tokio::select! {
                biased;

                // use update in place channel instead of common server reload channel for local config reload
                ev = cfg_receiver.changed() => {
                    if ev.is_err() {
                        warn!("SRT[{}_v{}#{}] quit as cfg channel closed",
                            self.server.name(), self.server_version, self.instance_id);
                        self.goto_close(listener);
                        break;
                    }
                    let value = cfg_receiver.borrow().clone();
                    match value {
                        None => {
                            info!("SRT[{}_v{}#{}] will go offline",
                                self.server.name(), self.server_version, self.instance_id);
                            self.pre_stop();
                            self.goto_offline(listener, listen_addr);
                            break;
                        }
                        Some(config) => {
                            aux_config = config;
                            if aux_config.next_server().ne(&next_server_name) {
                                info!("SRT[{}_v{}#{}] will use next server '{}' instead of '{next_server_name}'",
                                    self.server.name(), self.server_version, self.instance_id, aux_config.next_server());
                                next_server_name = aux_config.next_server().clone();
                                reload_next_server = true;
                            }

                            if let Some(quinn_config) = aux_config.take_quinn_config() {
                                listener.set_server_config(Some(quinn_config));
                            }
                            if let Some(listen_config) = aux_config.take_udp_listen_config() {
                                self.listen_config = listen_config;
                                if self.listen_config.address() != listen_addr {
                                    if let Ok((fd, addr)) = self.rebind_socket(&listener) {
                                        sock_raw_fd = fd;
                                        listen_addr = addr;
                                    }
                                } else {
                                    self.update_socket_opts(sock_raw_fd);
                                }
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
                        Ok(ServerReloadCommand::ReloadEscaper) => {
                            let escaper_name = next_server.escaper();
                            info!("SRT[{}_v{}#{}] will reload escaper {escaper_name}",
                                self.server.name(), self.server_version, self.instance_id);
                            run_ctx.update_escaper(escaper_name);
                        },
                        Ok(ServerReloadCommand::ReloadUserGroup) => {
                            let user_group_name = next_server.user_group();
                            info!("SRT[{}_v{}#{}] will reload user group {user_group_name}",
                                self.server.name(), self.server_version, self.instance_id);
                            run_ctx.update_user_group(user_group_name);
                        },
                        Ok(ServerReloadCommand::ReloadAuditor) => {
                            let auditor_name = next_server.auditor();
                            info!("SRT[{}_v{}#{}] will reload auditor {auditor_name}",
                                self.server.name(), self.server_version, self.instance_id);
                            run_ctx.update_audit_handle(auditor_name);
                        },
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
                    let Some(connecting) = result else {
                        continue;
                    };

                    let peer_addr = connecting.remote_address();
                    let local_addr = connecting
                        .local_ip()
                        .map(|ip| SocketAddr::new(ip, listen_addr.port()))
                        .unwrap_or(listen_addr);
                    let cc_info = ClientConnectionInfo::new(
                        native_socket_addr(peer_addr),
                        native_socket_addr(local_addr),
                    );
                    let (rt_handle, worker_id) = self.rt_handle();
                    let mut run_ctx = run_ctx.clone();
                    run_ctx.worker_id = worker_id;

                    aux_config.run_quic_task(rt_handle, next_server.clone(), connecting, cc_info, run_ctx);
                }
            }

            if reload_next_server {
                let result = crate::serve::get_with_notifier(&next_server_name);
                next_server = result.0;
                next_server_reload_channel = result.1;

                // if escaper changed, reload it
                let old_escaper = run_ctx.current_escaper();
                let new_escaper = next_server.escaper();
                if old_escaper.ne(new_escaper) {
                    info!("SRT[{}_v{}#{}] will use escaper '{new_escaper}' instead of '{old_escaper}'",
                                        self.server.name(), self.server_version, self.instance_id);
                    run_ctx.update_escaper(new_escaper);
                }

                // if user group changed, reload it
                let old_user_group = run_ctx.current_user_group();
                let new_user_group = next_server.user_group();
                if old_user_group.ne(new_user_group) {
                    info!("SRT[{}_v{}#{}] will use user group '{new_user_group}' instead of '{old_user_group}'",
                                        self.server.name(), self.server_version, self.instance_id);
                    run_ctx.update_user_group(new_user_group);
                }

                // if auditor changed, reload it
                let old_auditor = run_ctx.current_auditor();
                let new_auditor = next_server.auditor();
                if old_auditor.ne(new_auditor) {
                    info!("SRT[{}_v{}#{}] will use auditor '{new_auditor}' instead of '{old_auditor}'",
                                        self.server.name(), self.server_version, self.instance_id);
                    run_ctx.update_audit_handle(new_auditor);
                }
            }
        }
        self.post_stop();
    }

    fn update_socket_opts(&self, raw_fd: RawFd) {
        if let Err(e) = g3_socket::udp::set_raw_opts(raw_fd, &self.listen_config.socket_misc_opts())
        {
            warn!(
                "SRT[{}_v{}#{}] update socket misc opts failed: {e}",
                self.server.name(),
                self.server_version,
                self.instance_id,
            );
        }
        if let Err(e) = g3_socket::udp::set_raw_buf_opts(raw_fd, self.listen_config.socket_buffer())
        {
            warn!(
                "SRT[{}_v{}#{}] update socket buf opts failed: {e}",
                self.server.name(),
                self.server_version,
                self.instance_id,
            );
        }
    }

    fn rebind_socket(&self, listener: &Endpoint) -> io::Result<(RawFd, SocketAddr)> {
        match g3_socket::udp::new_std_bind_listen(&self.listen_config) {
            Ok(socket) => {
                let raw_fd = socket.as_raw_fd();
                match listener.rebind(socket) {
                    Ok(_) => Ok((raw_fd, listener.local_addr().unwrap())),
                    Err(e) => {
                        warn!(
                            "SRT[{}_v{}#{}] reload rebind {} failed: {e}",
                            self.server.name(),
                            self.server_version,
                            self.instance_id,
                            self.listen_config.address()
                        );
                        Err(e)
                    }
                }
            }
            Err(e) => {
                warn!(
                    "SRT[{}_v{}#{}] reload create new socket {} failed: {e}",
                    self.server.name(),
                    self.server_version,
                    self.instance_id,
                    self.listen_config.address()
                );
                Err(e)
            }
        }
    }

    fn goto_offline(&self, listener: Endpoint, listen_addr: SocketAddr) {
        if let Some(port) = self.rebind_port {
            let rebind_addr = SocketAddr::new(listen_addr.ip(), port);
            match g3_socket::udp::new_std_rebind_listen(
                &self.listen_config,
                SocketAddr::new(listen_addr.ip(), port),
            ) {
                Ok(socket) => match listener.rebind(socket) {
                    Ok(_) => {
                        info!(
                            "SRT[{}_v{}#{}] re-bound to: {rebind_addr}",
                            self.server.name(),
                            self.server_version,
                            self.instance_id
                        );
                        listener.reject_new_connections();
                        tokio::spawn(async move { listener.wait_idle().await });
                        return;
                    }
                    Err(e) => {
                        warn!(
                            "SRT[{}_v{}#{}] rebind failed: {e}",
                            self.server.name(),
                            self.server_version,
                            self.instance_id
                        );
                    }
                },
                Err(e) => {
                    warn!(
                        "SRT[{}_v{}#{}] create rebind socket failed: {e}",
                        self.server.name(),
                        self.server_version,
                        self.instance_id
                    );
                }
            }
        }
        self.goto_close(listener);
    }

    fn goto_close(&self, listener: Endpoint) {
        info!(
            "SRT[{}_v{}#{}] will close all quic connections immediately",
            self.server.name(),
            self.server_version,
            self.instance_id
        );
        listener.close(quinn::VarInt::default(), b"close as server shutdown");
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
        socket: UdpSocket,
        listen_addr: SocketAddr,
        config: quinn::ServerConfig,
        listen_in_worker: bool,
        cfg_receiver: watch::Receiver<Option<C>>,
    ) where
        C: AuxiliaryServerConfig + Clone + Send + Sync + 'static,
    {
        let handle = self.get_rt_handle(listen_in_worker);
        handle.spawn(async move {
            let sock_raw_fd = socket.as_raw_fd();
            // make sure the listen socket associated with the correct reactor
            match Endpoint::new(
                Default::default(),
                Some(config),
                socket,
                Arc::new(quinn::TokioRuntime),
            ) {
                Ok(endpoint) => {
                    self.pre_start();
                    self.run(endpoint, listen_addr, sock_raw_fd, cfg_receiver)
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
        listen_in_worker: bool,
        quic_config: &quinn::ServerConfig,
        cfg_receiver: &watch::Sender<Option<C>>,
    ) -> anyhow::Result<()>
    where
        C: AuxiliaryServerConfig + Clone + Send + Sync + 'static,
    {
        let mut instance_count = self.listen_config.instance();
        if listen_in_worker {
            let worker_count = g3_daemon::runtime::worker::worker_count();
            if worker_count > 0 {
                instance_count = worker_count;
            }
        }

        for i in 0..instance_count {
            let mut runtime = self.clone();
            runtime.instance_id = i;

            let socket = g3_socket::udp::new_std_bind_listen(&self.listen_config)?;
            let listen_addr = socket.local_addr()?;
            runtime.into_running(
                socket,
                listen_addr,
                quic_config.clone(),
                listen_in_worker,
                cfg_receiver.subscribe(),
            );
        }
        Ok(())
    }
}
