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
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use log::debug;
use slog::Logger;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc};
use tokio_rustls::server::TlsStream;
use tokio_rustls::LazyConfigAcceptor;

use g3_daemon::listen::ListenStats;
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::metrics::MetricsName;
use g3_types::net::{RustlsServerConfig, UpstreamAddr};
use g3_types::route::HostMatch;

use super::task::{
    CommonTaskContext, HttpRProxyPipelineReaderTask, HttpRProxyPipelineStats,
    HttpRProxyPipelineWriterTask,
};
use super::{HttpHost, HttpRProxyServerStats};
use crate::config::server::http_rproxy::HttpRProxyServerConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::escape::ArcEscaper;
use crate::serve::{
    ArcServer, ArcServerStats, OrdinaryTcpServerRuntime, Server, ServerInternal, ServerQuitPolicy,
    ServerReloadCommand, ServerRunContext, ServerStats,
};

pub(crate) struct HttpRProxyServer {
    config: Arc<HttpRProxyServerConfig>,
    server_stats: Arc<HttpRProxyServerStats>,
    listen_stats: Arc<ListenStats>,
    global_tls_server: Option<RustlsServerConfig>,
    ingress_net_filter: Option<Arc<AclNetworkRule>>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
    task_logger: Logger,
    hosts: HostMatch<Arc<HttpHost>>,

    quit_policy: Arc<ServerQuitPolicy>,
    reload_version: usize,
}

impl HttpRProxyServer {
    fn new(
        config: Arc<HttpRProxyServerConfig>,
        server_stats: Arc<HttpRProxyServerStats>,
        listen_stats: Arc<ListenStats>,
        hosts: HostMatch<Arc<HttpHost>>,
        version: usize,
    ) -> anyhow::Result<Self> {
        let (reload_sender, _reload_receiver) = crate::serve::new_reload_notify_channel();

        let global_tls_server = match &config.global_tls_server {
            Some(builder) => {
                let config = builder
                    .build()
                    .context("failed to build global tls server config")?;
                Some(config)
            }
            None => None,
        };

        let ingress_net_filter = config
            .ingress_net_filter
            .as_ref()
            .map(|builder| Arc::new(builder.build()));

        let task_logger = config.get_task_logger();

        // always update extra metrics tags
        server_stats.set_extra_tags(config.extra_metrics_tags.clone());

        let server = HttpRProxyServer {
            config,
            server_stats,
            listen_stats,
            global_tls_server,
            ingress_net_filter,
            reload_sender,
            task_logger,
            hosts,
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            reload_version: version,
        };

        Ok(server)
    }

    pub(crate) fn prepare_initial(config: AnyServerConfig) -> anyhow::Result<ArcServer> {
        if let AnyServerConfig::HttpRProxy(config) = config {
            let config = Arc::new(*config);
            let server_stats = Arc::new(HttpRProxyServerStats::new(config.name()));
            let listen_stats = Arc::new(ListenStats::new(config.name()));

            let hosts = (&config.hosts).try_into()?;

            let server = HttpRProxyServer::new(config, server_stats, listen_stats, hosts, 1)?;
            Ok(Arc::new(server))
        } else {
            Err(anyhow!("invalid config type for HttpRProxy server"))
        }
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<HttpRProxyServer> {
        if let AnyServerConfig::HttpRProxy(config) = config {
            let config = Arc::new(*config);
            let server_stats = Arc::clone(&self.server_stats);
            let listen_stats = Arc::clone(&self.listen_stats);

            // TODO do update if host has runtime state
            let hosts = (&config.hosts).try_into()?;

            let server = HttpRProxyServer::new(
                config,
                server_stats,
                listen_stats,
                hosts,
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

    fn get_common_task_context(
        &self,
        local_addr: SocketAddr,
        peer_addr: SocketAddr,
        escaper: ArcEscaper,
        worker_id: Option<usize>,
        raw_socket: RawFd,
    ) -> Arc<CommonTaskContext> {
        Arc::new(CommonTaskContext {
            server_config: Arc::clone(&self.config),
            server_stats: Arc::clone(&self.server_stats),
            server_quit_policy: Arc::clone(&self.quit_policy),
            escaper,
            tcp_server_addr: local_addr,
            tcp_client_addr: peer_addr,
            task_logger: self.task_logger.clone(),
            worker_id,
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
            run_ctx.worker_id,
            stream.as_raw_fd(),
        );
        let pipeline_stats = Arc::new(HttpRProxyPipelineStats::default());
        let (task_sender, task_receiver) = mpsc::channel(ctx.server_config.pipeline_size);

        // NOTE tls underlying traffic is not counted in (server/task/user) stats

        let (clt_r, clt_w) = tokio::io::split(stream);
        let r_task = HttpRProxyPipelineReaderTask::new(&ctx, task_sender, clt_r, &pipeline_stats);
        let w_task = HttpRProxyPipelineWriterTask::new(
            &ctx,
            run_ctx.user_group,
            task_receiver,
            clt_w,
            &pipeline_stats,
        );

        tokio::spawn(r_task.into_running());
        w_task.into_running(&self.hosts).await
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
            run_ctx.worker_id,
            stream.as_raw_fd(),
        );
        let pipeline_stats = Arc::new(HttpRProxyPipelineStats::default());
        let (task_sender, task_receiver) = mpsc::channel(ctx.server_config.pipeline_size);

        let (clt_r, clt_w) = stream.into_split();
        let r_task = HttpRProxyPipelineReaderTask::new(&ctx, task_sender, clt_r, &pipeline_stats);
        let w_task = HttpRProxyPipelineWriterTask::new(
            &ctx,
            run_ctx.user_group,
            task_receiver,
            clt_w,
            &pipeline_stats,
        );

        tokio::spawn(r_task.into_running());
        w_task.into_running(&self.hosts).await
    }
}

impl ServerInternal for HttpRProxyServer {
    fn _clone_config(&self) -> AnyServerConfig {
        AnyServerConfig::HttpRProxy(Box::new(self.config.as_ref().clone()))
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
impl Server for HttpRProxyServer {
    #[inline]
    fn name(&self) -> &MetricsName {
        self.config.name()
    }

    #[inline]
    fn version(&self) -> usize {
        self.reload_version
    }

    fn escaper(&self) -> &MetricsName {
        self.config.escaper()
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

        if self.config.enable_tls_server {
            let tls_acceptor = LazyConfigAcceptor::new(rustls::server::Acceptor::default(), stream);
            match tokio::time::timeout(self.config.client_hello_recv_timeout, tls_acceptor).await {
                Ok(Ok(start)) => {
                    let ch = start.client_hello();
                    let host = match ch.server_name() {
                        Some(host) => match UpstreamAddr::from_str(host) {
                            Ok(upstream) => self.hosts.get(upstream.host()),
                            Err(_) => self.hosts.get_default(),
                        },
                        None => self.hosts.get_default(),
                    };

                    match host
                        .and_then(|c| c.tls_server.as_ref())
                        .or(self.global_tls_server.as_ref())
                    {
                        Some(tls_config) => {
                            match tokio::time::timeout(
                                tls_config.accept_timeout,
                                start.into_stream(Arc::clone(&tls_config.driver)),
                            )
                            .await
                            {
                                Ok(Ok(stream)) => {
                                    self.spawn_tls_task(stream, peer_addr, local_addr, ctx)
                                        .await
                                }
                                Ok(Err(e)) => {
                                    self.listen_stats.add_failed();
                                    debug!("{local_addr} - {peer_addr} tls error: {e:?}");
                                    // TODO record tls failure and add some sec policy
                                }
                                Err(_) => {
                                    self.listen_stats.add_timeout();
                                    debug!("{local_addr} - {peer_addr} tls timeout");
                                    // TODO record tls failure and add some sec policy
                                }
                            }
                        }
                        None => {
                            // No tls server config found
                            self.listen_stats.add_failed();
                            debug!(
                                "{local_addr} - {peer_addr} tls error: no matched server config found",
                            );
                        }
                    }
                }
                Ok(Err(e)) => {
                    self.listen_stats.add_failed();
                    debug!("{local_addr} - {peer_addr} tls client hello error: {e:?}",);
                    // TODO record tls failure and add some sec policy
                }
                Err(_) => {
                    self.listen_stats.add_timeout();
                    debug!("{local_addr} - {peer_addr} tls client hello timeout");
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
