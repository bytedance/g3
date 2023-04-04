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
use std::os::unix::prelude::*;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use log::debug;
use slog::Logger;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc};
use tokio_rustls::{server::TlsStream, TlsAcceptor};

use g3_daemon::listen::ListenStats;
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::acl_set::AclDstHostRuleSet;
use g3_types::metrics::MetricsName;
use g3_types::net::OpensslTlsClientConfig;

use super::task::{
    CommonTaskContext, HttpProxyPipelineReaderTask, HttpProxyPipelineStats,
    HttpProxyPipelineWriterTask,
};
use super::HttpProxyServerStats;
use crate::audit::AuditHandle;
use crate::config::server::http_proxy::HttpProxyServerConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::escape::ArcEscaper;
use crate::serve::{
    ArcServer, ArcServerStats, OrdinaryTcpServerRuntime, Server, ServerInternal, ServerQuitPolicy,
    ServerReloadCommand, ServerRunContext, ServerStats,
};

pub(crate) struct HttpProxyServer {
    config: Arc<HttpProxyServerConfig>,
    server_stats: Arc<HttpProxyServerStats>,
    listen_stats: Arc<ListenStats>,
    tls_acceptor: Option<TlsAcceptor>,
    tls_accept_timeout: Duration,
    tls_client_config: Arc<OpensslTlsClientConfig>,
    ingress_net_filter: Option<Arc<AclNetworkRule>>,
    dst_host_filter: Option<Arc<AclDstHostRuleSet>>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
    task_logger: Logger,

    quit_policy: Arc<ServerQuitPolicy>,
    reload_version: usize,
}

impl HttpProxyServer {
    fn new(
        config: Arc<HttpProxyServerConfig>,
        server_stats: Arc<HttpProxyServerStats>,
        listen_stats: Arc<ListenStats>,
        version: usize,
    ) -> anyhow::Result<HttpProxyServer> {
        let (reload_sender, _reload_receiver) = crate::serve::new_reload_notify_channel();

        let mut tls_accept_timeout = Duration::from_secs(10);
        let tls_acceptor = if let Some(tls_config_builder) = &config.server_tls_config {
            let tls_server_config = tls_config_builder
                .build()
                .context("failed to build tls server config")?;
            tls_accept_timeout = tls_server_config.accept_timeout;
            Some(TlsAcceptor::from(tls_server_config.driver))
        } else {
            None
        };

        let tls_client_config = config
            .client_tls_config
            .build()
            .context("failed to build tls client config")?;

        let ingress_net_filter = config
            .ingress_net_filter
            .as_ref()
            .map(|builder| Arc::new(builder.build()));

        let dst_host_filter = config
            .dst_host_filter
            .as_ref()
            .map(|builder| Arc::new(builder.build()));

        let task_logger = config.get_task_logger();

        // always update extra metrics tags
        server_stats.set_extra_tags(config.extra_metrics_tags.clone());

        let server = HttpProxyServer {
            config,
            server_stats,
            listen_stats,
            tls_acceptor,
            tls_accept_timeout,
            tls_client_config: Arc::new(tls_client_config),
            ingress_net_filter,
            dst_host_filter,
            reload_sender,
            task_logger,
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            reload_version: version,
        };

        Ok(server)
    }

    pub(crate) fn prepare_initial(config: AnyServerConfig) -> anyhow::Result<ArcServer> {
        if let AnyServerConfig::HttpProxy(config) = config {
            let config = Arc::new(*config);
            let server_stats = Arc::new(HttpProxyServerStats::new(config.name()));
            let listen_stats = Arc::new(ListenStats::new(config.name()));

            let server = HttpProxyServer::new(config, server_stats, listen_stats, 1)?;
            Ok(Arc::new(server))
        } else {
            Err(anyhow!("invalid config type for HttpProxy server"))
        }
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<HttpProxyServer> {
        if let AnyServerConfig::HttpProxy(config) = config {
            let config = Arc::new(*config);
            let server_stats = Arc::clone(&self.server_stats);
            let listen_stats = Arc::clone(&self.listen_stats);

            let server =
                HttpProxyServer::new(config, server_stats, listen_stats, self.reload_version + 1)?;
            Ok(server)
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.server_type(),
                config.server_type()
            ))
        }
    }

    fn get_common_task_context(
        &self,
        local_addr: SocketAddr,
        peer_addr: SocketAddr,
        escaper: ArcEscaper,
        audit_handle: Option<Arc<AuditHandle>>,
        worker_id: Option<usize>,
        raw_socket: RawFd,
    ) -> Arc<CommonTaskContext> {
        Arc::new(CommonTaskContext {
            server_config: Arc::clone(&self.config),
            server_stats: Arc::clone(&self.server_stats),
            server_quit_policy: Arc::clone(&self.quit_policy),
            escaper,
            audit_handle,
            tcp_server_addr: local_addr,
            tcp_client_addr: peer_addr,
            tls_client_config: self.tls_client_config.clone(),
            task_logger: self.task_logger.clone(),
            worker_id,
            dst_host_filter: self.dst_host_filter.clone(),
            tcp_client_socket: raw_socket,
        })
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

    async fn spawn_tls_task(
        &self,
        stream: TlsStream<TcpStream>,
        peer_addr: SocketAddr,
        local_addr: SocketAddr,
        run_ctx: ServerRunContext,
    ) {
        let ctx = self.get_common_task_context(
            local_addr,
            peer_addr,
            run_ctx.escaper,
            run_ctx.audit_handle,
            run_ctx.worker_id,
            stream.as_raw_fd(),
        );
        let pipeline_stats = Arc::new(HttpProxyPipelineStats::default());
        let (task_sender, task_receiver) = mpsc::channel(ctx.server_config.pipeline_size);

        // NOTE tls underlying traffic is not counted in (server/task/user) stats

        let (clt_r, clt_w) = tokio::io::split(stream);
        let r_task = HttpProxyPipelineReaderTask::new(&ctx, task_sender, clt_r, &pipeline_stats);
        let w_task = HttpProxyPipelineWriterTask::new(
            &ctx,
            run_ctx.user_group,
            task_receiver,
            clt_w,
            &pipeline_stats,
        );

        tokio::spawn(r_task.into_running());
        w_task.into_running().await
    }

    async fn spawn_tcp_task(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
        local_addr: SocketAddr,
        run_ctx: ServerRunContext,
    ) {
        let ctx = self.get_common_task_context(
            local_addr,
            peer_addr,
            run_ctx.escaper,
            run_ctx.audit_handle,
            run_ctx.worker_id,
            stream.as_raw_fd(),
        );
        let pipeline_stats = Arc::new(HttpProxyPipelineStats::default());
        let (task_sender, task_receiver) = mpsc::channel(ctx.server_config.pipeline_size);

        let (clt_r, clt_w) = stream.into_split();
        let r_task = HttpProxyPipelineReaderTask::new(&ctx, task_sender, clt_r, &pipeline_stats);
        let w_task = HttpProxyPipelineWriterTask::new(
            &ctx,
            run_ctx.user_group,
            task_receiver,
            clt_w,
            &pipeline_stats,
        );

        tokio::spawn(r_task.into_running());
        w_task.into_running().await
    }
}

impl ServerInternal for HttpProxyServer {
    fn _clone_config(&self) -> AnyServerConfig {
        AnyServerConfig::HttpProxy(Box::new(self.config.as_ref().clone()))
    }

    fn _update_config_in_place(&self, _flags: u64, _config: AnyServerConfig) -> anyhow::Result<()> {
        Ok(())
    }

    fn _get_reload_notifier(&self) -> broadcast::Receiver<ServerReloadCommand> {
        self.reload_sender.subscribe()
    }

    fn _reload_config_notify_runtime(&self) {
        let cmd = ServerReloadCommand::ReloadVersion(self.reload_version);
        let _ = self.reload_sender.send(cmd);
    }

    fn _reload_escaper_notify_runtime(&self) {
        let _ = self.reload_sender.send(ServerReloadCommand::ReloadEscaper);
    }

    fn _reload_user_group_notify_runtime(&self) {
        let _ = self
            .reload_sender
            .send(ServerReloadCommand::ReloadUserGroup);
    }

    fn _reload_auditor_notify_runtime(&self) {
        let _ = self.reload_sender.send(ServerReloadCommand::ReloadAuditor);
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
        let runtime = OrdinaryTcpServerRuntime::new(server, &*self.config);
        runtime
            .run_all_instances(
                &self.config.listen,
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

#[async_trait]
impl Server for HttpProxyServer {
    #[inline]
    fn name(&self) -> &str {
        self.config.name()
    }

    #[inline]
    fn version(&self) -> usize {
        self.reload_version
    }

    fn escaper(&self) -> String {
        self.config.escaper().to_string()
    }

    fn user_group(&self) -> &MetricsName {
        self.config.user_group()
    }

    fn auditor(&self) -> &MetricsName {
        self.config.auditor()
    }

    fn get_server_stats(&self) -> Option<ArcServerStats> {
        Some(Arc::clone(&self.server_stats) as ArcServerStats)
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

    async fn run_tcp_task(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
        local_addr: SocketAddr,
        ctx: ServerRunContext,
    ) {
        self.server_stats.add_conn(peer_addr);

        if self.drop_early(peer_addr) {
            return;
        }

        if let Some(tls_acceptor) = &self.tls_acceptor {
            match tokio::time::timeout(self.tls_accept_timeout, tls_acceptor.accept(stream)).await {
                Ok(Ok(tls_stream)) => {
                    self.spawn_tls_task(tls_stream, peer_addr, local_addr, ctx)
                        .await
                }
                Ok(Err(e)) => {
                    self.listen_stats.add_failed();
                    debug!("{} - {} tls error: {:?}", local_addr, peer_addr, e);
                    // TODO record tls failure and add some sec policy
                }
                Err(_) => {
                    self.listen_stats.add_timeout();
                    debug!("{} - {} tls timeout", local_addr, peer_addr);
                    // TODO record tls failure and add some sec policy
                }
            }
        } else {
            self.spawn_tcp_task(stream, peer_addr, local_addr, ctx)
                .await;
        }
    }

    async fn run_tls_task(
        &self,
        stream: TlsStream<TcpStream>,
        peer_addr: SocketAddr,
        local_addr: SocketAddr,
        ctx: ServerRunContext,
    ) {
        self.server_stats.add_conn(peer_addr);

        if self.drop_early(peer_addr) {
            return;
        }
        self.spawn_tls_task(stream, peer_addr, local_addr, ctx)
            .await;
    }
}
