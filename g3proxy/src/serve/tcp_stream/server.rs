/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, anyhow};
use arc_swap::{ArcSwap, ArcSwapOption};
use async_trait::async_trait;
#[cfg(feature = "quic")]
use quinn::Connection;
use slog::Logger;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_rustls::server::TlsStream;

use g3_daemon::listen::{AcceptQuicServer, AcceptTcpServer, ListenStats, ListenTcpRuntime};
use g3_daemon::server::{BaseServer, ClientConnectionInfo, ServerExt, ServerReloadCommand};
use g3_io_ext::{AsyncStream, IdleWheel};
use g3_openssl::SslStream;
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::collection::{SelectiveVec, SelectiveVecBuilder};
use g3_types::metrics::NodeName;
use g3_types::net::{OpensslClientConfig, UpstreamAddr, WeightedUpstreamAddr};

use super::common::CommonTaskContext;
use super::stats::TcpStreamServerStats;
use super::task::TcpStreamTask;
use crate::audit::{AuditContext, AuditHandle};
use crate::config::server::tcp_stream::TcpStreamServerConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::escape::ArcEscaper;
use crate::serve::{
    ArcServer, ArcServerInternal, ArcServerStats, Server, ServerInternal, ServerQuitPolicy,
    ServerRegistry, ServerStats, WrapArcServer,
};

pub(crate) struct TcpStreamServer {
    config: Arc<TcpStreamServerConfig>,
    server_stats: Arc<TcpStreamServerStats>,
    listen_stats: Arc<ListenStats>,
    upstream: SelectiveVec<WeightedUpstreamAddr>,
    tls_client_config: Option<Arc<OpensslClientConfig>>,
    ingress_net_filter: Option<AclNetworkRule>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
    task_logger: Option<Logger>,

    escaper: ArcSwap<ArcEscaper>,
    audit_handle: ArcSwapOption<AuditHandle>,
    quit_policy: Arc<ServerQuitPolicy>,
    idle_wheel: Arc<IdleWheel>,
    reload_version: usize,
}

impl TcpStreamServer {
    fn new(
        config: Arc<TcpStreamServerConfig>,
        server_stats: Arc<TcpStreamServerStats>,
        listen_stats: Arc<ListenStats>,
        version: usize,
    ) -> anyhow::Result<TcpStreamServer> {
        let reload_sender = crate::serve::new_reload_notify_channel();

        let mut nodes_builder = SelectiveVecBuilder::new();
        for node in &config.upstream {
            nodes_builder.insert(node.clone());
        }
        let upstream = nodes_builder
            .build()
            .ok_or_else(|| anyhow!("no upstream addr set"))?;

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
        let idle_wheel = IdleWheel::spawn(config.task_idle_check_interval);

        server_stats.set_extra_tags(config.extra_metrics_tags.clone());

        let escaper = Arc::new(crate::escape::get_or_insert_default(config.escaper()));
        let audit_handle = config.get_audit_handle()?;

        let server = TcpStreamServer {
            config,
            server_stats,
            listen_stats,
            upstream,
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

    pub(crate) fn prepare_initial(
        config: TcpStreamServerConfig,
    ) -> anyhow::Result<ArcServerInternal> {
        let config = Arc::new(config);
        let server_stats = Arc::new(TcpStreamServerStats::new(config.name()));
        let listen_stats = Arc::new(ListenStats::new(config.name()));

        let server = TcpStreamServer::new(config, server_stats, listen_stats, 1)?;
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<TcpStreamServer> {
        if let AnyServerConfig::TcpStream(config) = config {
            let config = Arc::new(config);
            let server_stats = Arc::clone(&self.server_stats);
            let listen_stats = Arc::clone(&self.listen_stats);

            let server =
                TcpStreamServer::new(config, server_stats, listen_stats, self.reload_version + 1)?;
            Ok(server)
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.r#type(),
                config.r#type()
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

    fn get_ctx_and_upstream(
        &self,
        cc_info: ClientConnectionInfo,
    ) -> (CommonTaskContext, &UpstreamAddr) {
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

        (ctx, upstream.inner())
    }

    async fn run_task_with_stream<T>(&self, stream: T, cc_info: ClientConnectionInfo)
    where
        T: AsyncStream,
        T::R: AsyncRead + Send + Sync + Unpin + 'static,
        T::W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let (ctx, upstream) = self.get_ctx_and_upstream(cc_info);

        let (clt_r, clt_w) = stream.into_split();
        TcpStreamTask::new(ctx, upstream, self.audit_context())
            .into_running(clt_r, clt_w)
            .await;
    }

    #[cfg(feature = "quic")]
    fn run_task_with_quic_stream(
        &self,
        send_stream: quinn::SendStream,
        recv_stream: quinn::RecvStream,
        cc_info: ClientConnectionInfo,
    ) {
        let (ctx, upstream) = self.get_ctx_and_upstream(cc_info);

        tokio::spawn(
            TcpStreamTask::new(ctx, upstream, self.audit_context())
                .into_running(recv_stream, send_stream),
        );
    }
}

impl ServerInternal for TcpStreamServer {
    fn _clone_config(&self) -> AnyServerConfig {
        AnyServerConfig::TcpStream(self.config.as_ref().clone())
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

    fn _reload_with_old_notifier(
        &self,
        config: AnyServerConfig,
        _registry: &mut ServerRegistry,
    ) -> anyhow::Result<ArcServerInternal> {
        let mut server = self.prepare_reload(config)?;
        server.reload_sender = self.reload_sender.clone();
        Ok(Arc::new(server))
    }

    fn _reload_with_new_notifier(
        &self,
        config: AnyServerConfig,
        _registry: &mut ServerRegistry,
    ) -> anyhow::Result<ArcServerInternal> {
        let server = self.prepare_reload(config)?;
        Ok(Arc::new(server))
    }

    fn _start_runtime(&self, server: ArcServer) -> anyhow::Result<()> {
        let Some(listen_config) = &self.config.listen else {
            return Ok(());
        };
        let listen_stats = server.get_listen_stats();
        let runtime = ListenTcpRuntime::new(WrapArcServer(server), listen_stats);
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

impl BaseServer for TcpStreamServer {
    #[inline]
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    #[inline]
    fn r#type(&self) -> &'static str {
        self.config.r#type()
    }

    #[inline]
    fn version(&self) -> usize {
        self.reload_version
    }
}

impl ServerExt for TcpStreamServer {}

#[async_trait]
impl AcceptTcpServer for TcpStreamServer {
    async fn run_tcp_task(&self, stream: TcpStream, cc_info: ClientConnectionInfo) {
        let client_addr = cc_info.client_addr();
        self.server_stats.add_conn(client_addr);
        if self.drop_early(client_addr) {
            return;
        }

        self.run_task_with_stream(stream, cc_info).await
    }
}

#[async_trait]
impl AcceptQuicServer for TcpStreamServer {
    #[cfg(feature = "quic")]
    async fn run_quic_task(&self, connection: Connection, cc_info: ClientConnectionInfo) {
        use log::debug;

        let client_addr = cc_info.client_addr();
        self.server_stats.add_conn(client_addr);
        if self.drop_early(client_addr) {
            return;
        }

        loop {
            // TODO update ctx and quit gracefully
            match connection.accept_bi().await {
                Ok((send_stream, recv_stream)) => {
                    self.run_task_with_quic_stream(send_stream, recv_stream, cc_info.clone())
                }
                Err(e) => {
                    debug!(
                        "{} - {} quic connection error: {e:?}",
                        cc_info.sock_local_addr(),
                        cc_info.sock_peer_addr()
                    );
                    break;
                }
            }
        }
    }
}

#[async_trait]
impl Server for TcpStreamServer {
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

    async fn run_rustls_task(&self, stream: TlsStream<TcpStream>, cc_info: ClientConnectionInfo) {
        let client_addr = cc_info.client_addr();
        self.server_stats.add_conn(client_addr);
        if self.drop_early(client_addr) {
            return;
        }

        self.run_task_with_stream(stream, cc_info).await
    }

    async fn run_openssl_task(&self, stream: SslStream<TcpStream>, cc_info: ClientConnectionInfo) {
        let client_addr = cc_info.client_addr();
        self.server_stats.add_conn(client_addr);
        if self.drop_early(client_addr) {
            return;
        }

        self.run_task_with_stream(stream, cc_info).await
    }
}
