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
use std::time::Duration;

use anyhow::{anyhow, Context};
use arc_swap::{ArcSwap, ArcSwapOption};
use async_trait::async_trait;
use log::debug;
#[cfg(feature = "quic")]
use quinn::Connection;
use slog::Logger;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_rustls::{server::TlsStream, TlsAcceptor};

use g3_daemon::listen::{AcceptQuicServer, AcceptTcpServer, ListenStats, ListenTcpRuntime};
use g3_daemon::server::{BaseServer, ClientConnectionInfo, ServerExt, ServerReloadCommand};
use g3_io_ext::IdleWheel;
use g3_openssl::SslStream;
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::collection::{SelectiveVec, SelectiveVecBuilder};
use g3_types::metrics::NodeName;
use g3_types::net::{
    OpensslClientConfig, OpensslTicketKey, RollingTicketer, RustlsServerConnectionExt,
    WeightedUpstreamAddr,
};

use super::common::CommonTaskContext;
use super::task::TlsStreamTask;
use crate::audit::{AuditContext, AuditHandle};
use crate::config::server::tls_stream::TlsStreamServerConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::escape::ArcEscaper;
use crate::serve::tcp_stream::TcpStreamServerStats;
use crate::serve::{
    ArcServer, ArcServerStats, Server, ServerInternal, ServerQuitPolicy, ServerStats, WrapArcServer,
};

pub(crate) struct TlsStreamServer {
    config: Arc<TlsStreamServerConfig>,
    server_stats: Arc<TcpStreamServerStats>,
    listen_stats: Arc<ListenStats>,
    upstream: SelectiveVec<WeightedUpstreamAddr>,
    tls_rolling_ticketer: Option<Arc<RollingTicketer<OpensslTicketKey>>>,
    tls_acceptor: TlsAcceptor,
    tls_accept_timeout: Duration,
    tls_client_config: Option<Arc<OpensslClientConfig>>,
    ingress_net_filter: Option<AclNetworkRule>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
    task_logger: Logger,

    escaper: ArcSwap<ArcEscaper>,
    audit_handle: ArcSwapOption<AuditHandle>,
    quit_policy: Arc<ServerQuitPolicy>,
    idle_wheel: Arc<IdleWheel>,
    reload_version: usize,
}

impl TlsStreamServer {
    fn new(
        config: Arc<TlsStreamServerConfig>,
        server_stats: Arc<TcpStreamServerStats>,
        listen_stats: Arc<ListenStats>,
        tls_rolling_ticketer: Option<Arc<RollingTicketer<OpensslTicketKey>>>,
        version: usize,
    ) -> anyhow::Result<Self> {
        let reload_sender = crate::serve::new_reload_notify_channel();

        let mut nodes_builder = SelectiveVecBuilder::new();
        for node in &config.upstream {
            nodes_builder.insert(node.clone());
        }
        let upstream = nodes_builder
            .build()
            .ok_or_else(|| anyhow!("no upstream addr set"))?;

        let tls_server_config = config
            .server_tls_config
            .build_with_ticketer(tls_rolling_ticketer.clone())
            .context("failed to build tls server config")?;

        let tls_client_config = if let Some(builder) = &config.client_tls_config {
            let tls_config = builder
                .build()
                .context("failed to build tls client config")?;

            Some(Arc::new(tls_config))
        } else {
            None
        };

        let ingress_net_filter = config
            .ingress_net_filter
            .as_ref()
            .map(|builder| builder.build());

        let task_logger = config.get_task_logger();
        let idle_wheel = IdleWheel::spawn(config.task_idle_check_duration);

        server_stats.set_extra_tags(config.extra_metrics_tags.clone());

        let escaper = Arc::new(crate::escape::get_or_insert_default(config.escaper()));
        let audit_handle = config.get_audit_handle()?;

        let server = TlsStreamServer {
            config,
            server_stats,
            listen_stats,
            upstream,
            tls_rolling_ticketer,
            tls_acceptor: TlsAcceptor::from(tls_server_config.driver),
            tls_accept_timeout: tls_server_config.accept_timeout,
            tls_client_config,
            ingress_net_filter,
            reload_sender,
            task_logger,
            escaper: ArcSwap::new(escaper),
            audit_handle: ArcSwapOption::new(audit_handle),
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            idle_wheel,
            reload_version: version,
        };

        Ok(server)
    }

    pub(crate) fn prepare_initial(config: TlsStreamServerConfig) -> anyhow::Result<ArcServer> {
        let config = Arc::new(config);
        let server_stats = Arc::new(TcpStreamServerStats::new(config.name()));
        let listen_stats = Arc::new(ListenStats::new(config.name()));

        let tls_rolling_ticketer = if let Some(c) = &config.tls_ticketer {
            let ticketer = c
                .build_and_spawn_updater()
                .context("failed to create tls rolling ticketer")?;
            Some(ticketer)
        } else {
            None
        };

        let server =
            TlsStreamServer::new(config, server_stats, listen_stats, tls_rolling_ticketer, 1)?;
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<Self> {
        if let AnyServerConfig::TlsStream(config) = config {
            let config = Arc::new(*config);
            let server_stats = Arc::clone(&self.server_stats);
            let listen_stats = Arc::clone(&self.listen_stats);

            let tls_rolling_ticketer = if self.config.tls_ticketer.eq(&config.tls_ticketer) {
                self.tls_rolling_ticketer.clone()
            } else if let Some(c) = &config.tls_ticketer {
                let ticketer = c
                    .build_and_spawn_updater()
                    .context("failed to create tls rolling ticketer")?;
                Some(ticketer)
            } else {
                None
            };

            let server = TlsStreamServer::new(
                config,
                server_stats,
                listen_stats,
                tls_rolling_ticketer,
                self.reload_version + 1,
            )?;
            Ok(server)
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.server_type(),
                config.server_type()
            ))
        }
    }

    fn drop_early(&self, client_addr: SocketAddr) -> bool {
        if let Some(ingress_net_filter) = &self.ingress_net_filter {
            let (_, action) = ingress_net_filter.check(client_addr.ip());
            match action {
                AclAction::Permit | AclAction::PermitAndLog => {}
                AclAction::Forbid | AclAction::ForbidAndLog => {
                    self.listen_stats.add_dropped();
                    return true;
                }
            }
        }

        // TODO add cps limit

        false
    }

    fn audit_context(&self) -> AuditContext {
        AuditContext::new(self.audit_handle.load_full())
    }

    async fn run_task(&self, stream: TlsStream<TcpStream>, cc_info: ClientConnectionInfo) {
        let upstream =
            self.select_consistent(&self.upstream, self.config.upstream_pick_policy, &cc_info);

        let ctx = CommonTaskContext {
            server_config: self.config.clone(),
            server_stats: self.server_stats.clone(),
            server_quit_policy: self.quit_policy.clone(),
            idle_wheel: self.idle_wheel.clone(),
            escaper: self.escaper.load().as_ref().clone(),
            cc_info,
            tls_client_config: self.tls_client_config.clone(),
            task_logger: self.task_logger.clone(),
        };

        TlsStreamTask::new(ctx, upstream.inner(), self.audit_context())
            .into_running(stream)
            .await;
    }
}

impl ServerInternal for TlsStreamServer {
    fn _clone_config(&self) -> AnyServerConfig {
        AnyServerConfig::TlsStream(Box::new(self.config.as_ref().clone()))
    }

    fn _update_config_in_place(&self, _flags: u64, _config: AnyServerConfig) -> anyhow::Result<()> {
        Ok(())
    }

    fn _depend_on_server(&self, _name: &NodeName) -> bool {
        false
    }

    fn _reload_config_notify_runtime(&self) {
        let cmd = ServerReloadCommand::ReloadVersion(self.reload_version);
        let _ = self.reload_sender.send(cmd);
    }

    fn _update_next_servers_in_place(&self) {}

    fn _update_escaper_in_place(&self) {
        let escaper = crate::escape::get_or_insert_default(self.config.escaper());
        self.escaper.store(Arc::new(escaper));
    }

    fn _update_user_group_in_place(&self) {}

    fn _update_audit_handle_in_place(&self) -> anyhow::Result<()> {
        let audit_handle = self.config.get_audit_handle()?;
        self.audit_handle.store(audit_handle);
        Ok(())
    }

    fn _reload_with_old_notifier(&self, config: AnyServerConfig) -> anyhow::Result<ArcServer> {
        let mut server = self.prepare_reload(config)?;
        server.reload_sender = self.reload_sender.clone();
        Ok(Arc::new(server))
    }

    fn _reload_with_new_notifier(&self, config: AnyServerConfig) -> anyhow::Result<ArcServer> {
        let server = self.prepare_reload(config)?;
        Ok(Arc::new(server))
    }

    fn _start_runtime(&self, server: &ArcServer) -> anyhow::Result<()> {
        let Some(listen_config) = &self.config.listen else {
            return Ok(());
        };
        let runtime =
            ListenTcpRuntime::new(WrapArcServer(server.clone()), server.get_listen_stats());
        runtime
            .run_all_instances(
                listen_config,
                self.config.listen_in_worker,
                &self.reload_sender,
            )
            .map(|_| self.server_stats.set_online())
    }

    fn _abort_runtime(&self) {
        let _ = self.reload_sender.send(ServerReloadCommand::QuitRuntime);
        self.server_stats.set_offline();
    }
}

impl BaseServer for TlsStreamServer {
    #[inline]
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    #[inline]
    fn server_type(&self) -> &'static str {
        self.config.server_type()
    }

    #[inline]
    fn version(&self) -> usize {
        self.reload_version
    }
}

impl ServerExt for TlsStreamServer {}

#[async_trait]
impl AcceptTcpServer for TlsStreamServer {
    async fn run_tcp_task(&self, stream: TcpStream, cc_info: ClientConnectionInfo) {
        let client_addr = cc_info.client_addr();
        self.server_stats.add_conn(client_addr);
        if self.drop_early(client_addr) {
            return;
        }

        match tokio::time::timeout(self.tls_accept_timeout, self.tls_acceptor.accept(stream)).await
        {
            Ok(Ok(stream)) => {
                if stream.get_ref().1.session_reused() {
                    // Quick ACK is needed with session resumption
                    cc_info.tcp_sock_try_quick_ack();
                }
                self.run_task(stream, cc_info).await
            }
            Ok(Err(e)) => {
                self.listen_stats.add_failed();
                debug!(
                    "{} - {} tls error: {e:?}",
                    cc_info.sock_local_addr(),
                    cc_info.sock_peer_addr()
                );
                // TODO record tls failure and add some sec policy
            }
            Err(_) => {
                self.listen_stats.add_timeout();
                debug!(
                    "{} - {} tls timeout",
                    cc_info.sock_local_addr(),
                    cc_info.sock_peer_addr()
                );
                // TODO record tls failure and add some sec policy
            }
        }
    }
}

#[async_trait]
impl AcceptQuicServer for TlsStreamServer {
    #[cfg(feature = "quic")]
    async fn run_quic_task(&self, _connection: Connection, _cc_info: ClientConnectionInfo) {}
}

#[async_trait]
impl Server for TlsStreamServer {
    fn escaper(&self) -> &NodeName {
        self.config.escaper()
    }

    fn user_group(&self) -> &NodeName {
        Default::default()
    }

    fn auditor(&self) -> &NodeName {
        self.config.auditor()
    }

    fn get_server_stats(&self) -> Option<ArcServerStats> {
        Some(self.server_stats.clone())
    }

    fn get_listen_stats(&self) -> Arc<ListenStats> {
        Arc::clone(&self.listen_stats)
    }

    fn alive_count(&self) -> i32 {
        self.server_stats.get_alive_count()
    }

    #[inline]
    fn quit_policy(&self) -> &Arc<ServerQuitPolicy> {
        &self.quit_policy
    }

    async fn run_rustls_task(&self, _stream: TlsStream<TcpStream>, _cc_info: ClientConnectionInfo) {
    }

    async fn run_openssl_task(
        &self,
        _stream: SslStream<TcpStream>,
        _cc_info: ClientConnectionInfo,
    ) {
    }
}
