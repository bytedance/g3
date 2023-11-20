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
use std::time::Duration;

use log::{info, warn};
use quinn::{Connecting, Endpoint};
use tokio::runtime::Handle;
use tokio::sync::{broadcast, watch};

use g3_daemon::listen::{ListenQuicConf, ListenStats};
use g3_daemon::server::{ClientConnectionInfo, ServerReloadCommand};
use g3_socket::util::native_socket_addr;
use g3_types::acl::AclAction;
use g3_types::net::UdpListenConfig;

use crate::config::server::ServerConfig;
use crate::serve::ArcServer;

#[derive(Clone)]
pub(crate) struct ListenQuicRuntime {
    server: ArcServer,
    server_type: &'static str,
    server_version: usize,
    worker_id: Option<usize>,
    listen_config: UdpListenConfig,
    listen_stats: Arc<ListenStats>,
    instance_id: usize,
}

impl ListenQuicRuntime {
    pub(crate) fn new<C: ServerConfig>(
        server: &ArcServer,
        server_config: &C,
        listen_config: UdpListenConfig,
    ) -> Self {
        ListenQuicRuntime {
            server: Arc::clone(server),
            server_type: server_config.server_type(),
            server_version: server.version(),
            worker_id: None,
            listen_config,
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

    async fn run<C>(
        mut self,
        listener: Endpoint,
        mut listen_addr: SocketAddr,
        mut sock_raw_fd: RawFd,
        mut server_reload_channel: broadcast::Receiver<ServerReloadCommand>,
        mut quic_cfg_receiver: watch::Receiver<C>,
    ) where
        C: ListenQuicConf + Send + Clone + 'static,
    {
        use broadcast::error::RecvError;

        let mut aux_config = quic_cfg_receiver.borrow().clone();

        loop {
            tokio::select! {
                biased;

                ev = server_reload_channel.recv() => {
                   match ev {
                        Ok(ServerReloadCommand::ReloadVersion(version)) => {
                            info!("SRT[{}_v{}#{}] received reload request from v{version}",
                                self.server.name(), self.server_version, self.instance_id);
                            match crate::serve::get_server(self.server.name()) {
                                Ok(server) => {
                                    self.server_version = server.version();
                                    self.server = server;
                                    continue;
                                }
                                Err(_) => {
                                    info!("SRT[{}_v{}#{}] will quit as no server v{version}+ found",
                                        self.server.name(), self.server_version, self.instance_id);
                                }
                            }
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
                    self.goto_offline(listener, listen_addr, aux_config.offline_rebind_port());
                    break;
                }
                ev = quic_cfg_receiver.changed() => {
                    if ev.is_err() {
                        warn!("SRT[{}_v{}#{}] quit as quic cfg channel closed",
                            self.server.name(), self.server_version, self.instance_id);
                        self.goto_close(listener);
                        break;
                    }
                    aux_config = quic_cfg_receiver.borrow().clone();
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
                result = listener.accept() => {
                    let Some(connecting) = result else {
                        continue;
                    };
                    self.listen_stats.add_accepted();
                    self.run_task(connecting, listen_addr, &aux_config);
                }
            }
        }
        self.post_stop();
    }

    fn run_task<C>(&self, connecting: Connecting, listen_addr: SocketAddr, aux_config: &C)
    where
        C: ListenQuicConf + Send + Clone + 'static,
    {
        let peer_addr = connecting.remote_address();
        if let Some(filter) = aux_config.ingress_network_acl() {
            let (_, action) = filter.check(peer_addr.ip());
            match action {
                AclAction::Permit | AclAction::PermitAndLog => {}
                AclAction::Forbid | AclAction::ForbidAndLog => {
                    self.listen_stats.add_dropped();
                    return;
                }
            }
        }

        let local_addr = connecting
            .local_ip()
            .map(|ip| SocketAddr::new(ip, listen_addr.port()))
            .unwrap_or(listen_addr);
        let mut cc_info = ClientConnectionInfo::new(
            native_socket_addr(peer_addr),
            native_socket_addr(local_addr),
        );

        let server = self.server.clone();
        let listen_stats = self.listen_stats.clone();
        let accept_timeout = aux_config.accept_timeout();
        if let Some(worker_id) = self.worker_id {
            cc_info.set_worker_id(Some(worker_id));
            tokio::spawn(async move {
                Self::accept_connection_and_run(
                    server,
                    connecting,
                    cc_info,
                    accept_timeout,
                    listen_stats,
                )
                .await
            });
        } else if let Some(rt) = g3_daemon::runtime::worker::select_handle() {
            cc_info.set_worker_id(Some(rt.id));
            rt.handle.spawn(async move {
                Self::accept_connection_and_run(
                    server,
                    connecting,
                    cc_info,
                    accept_timeout,
                    listen_stats,
                )
                .await
            });
        } else {
            tokio::spawn(async move {
                Self::accept_connection_and_run(
                    server,
                    connecting,
                    cc_info,
                    accept_timeout,
                    listen_stats,
                )
                .await
            });
        }
    }

    async fn accept_connection_and_run(
        server: ArcServer,
        connecting: Connecting,
        cc_info: ClientConnectionInfo,
        timeout: Duration,
        listen_stats: Arc<ListenStats>,
    ) {
        match tokio::time::timeout(timeout, connecting).await {
            Ok(Ok(c)) => {
                listen_stats.add_accepted();
                server.run_quic_task(c, cc_info).await;
            }
            Ok(Err(_e)) => {
                listen_stats.add_failed();
                // TODO may be attack
            }
            Err(_) => {
                listen_stats.add_failed();
                // TODO may be attack
            }
        }
    }

    fn update_socket_opts(&self, raw_fd: RawFd) {
        if let Err(e) = g3_socket::udp::set_raw_opts(raw_fd, self.listen_config.socket_misc_opts())
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

    fn goto_offline(&self, listener: Endpoint, listen_addr: SocketAddr, rebind_port: Option<u16>) {
        if let Some(port) = rebind_port {
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
        server_reload_channel: broadcast::Receiver<ServerReloadCommand>,
        quic_cfg_receiver: watch::Receiver<C>,
    ) where
        C: ListenQuicConf + Clone + Send + Sync + 'static,
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
                    self.run(
                        endpoint,
                        listen_addr,
                        sock_raw_fd,
                        server_reload_channel,
                        quic_cfg_receiver,
                    )
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
        server_reload_sender: &broadcast::Sender<ServerReloadCommand>,
        quic_cfg_receiver: &watch::Sender<C>,
    ) -> anyhow::Result<()>
    where
        C: ListenQuicConf + Clone + Send + Sync + 'static,
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
                server_reload_sender.subscribe(),
                quic_cfg_receiver.subscribe(),
            );
        }
        Ok(())
    }
}
