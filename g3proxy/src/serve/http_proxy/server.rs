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

use anyhow::{Context, anyhow};
use arc_swap::{ArcSwap, ArcSwapOption};
use async_trait::async_trait;
use log::debug;
#[cfg(feature = "quic")]
use quinn::Connection;
use slog::Logger;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc};
use tokio_rustls::{TlsAcceptor, server::TlsStream};

use g3_daemon::listen::{AcceptQuicServer, AcceptTcpServer, ListenStats, ListenTcpRuntime};
use g3_daemon::server::{BaseServer, ClientConnectionInfo, ServerReloadCommand};
use g3_io_ext::{AsyncStream, IdleWheel};
use g3_openssl::SslStream;
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::acl_set::AclDstHostRuleSet;
use g3_types::metrics::NodeName;
use g3_types::net::{
    AlpnProtocol, OpensslClientConfig, OpensslTicketKey, RollingTicketer, RustlsServerConnectionExt,
};

use super::HttpProxyServerStats;
use super::task::{
    CommonTaskContext, HttpProxyPipelineReaderTask, HttpProxyPipelineStats,
    HttpProxyPipelineWriterTask,
};
use crate::audit::{AuditContext, AuditHandle};
use crate::auth::UserGroup;
use crate::config::server::http_proxy::HttpProxyServerConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::escape::ArcEscaper;
use crate::serve::{
    ArcServer, ArcServerStats, Server, ServerInternal, ServerQuitPolicy, ServerStats, WrapArcServer,
};

pub(crate) struct HttpProxyServer {
    config: Arc<HttpProxyServerConfig>,
    server_stats: Arc<HttpProxyServerStats>,
    listen_stats: Arc<ListenStats>,
    tls_rolling_ticketer: Option<Arc<RollingTicketer<OpensslTicketKey>>>,
    tls_acceptor: Option<TlsAcceptor>,
    tls_accept_timeout: Duration,
    tls_client_config: Arc<OpensslClientConfig>,
    ingress_net_filter: Option<AclNetworkRule>,
    dst_host_filter: Option<Arc<AclDstHostRuleSet>>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
    task_logger: Logger,

    escaper: ArcSwap<ArcEscaper>,
    user_group: ArcSwapOption<UserGroup>,
    audit_handle: ArcSwapOption<AuditHandle>,
    quit_policy: Arc<ServerQuitPolicy>,
    idle_wheel: Arc<IdleWheel>,
    reload_version: usize,
}

impl HttpProxyServer {
    fn new(
        config: Arc<HttpProxyServerConfig>,
        server_stats: Arc<HttpProxyServerStats>,
        listen_stats: Arc<ListenStats>,
        tls_rolling_ticketer: Option<Arc<RollingTicketer<OpensslTicketKey>>>,
        version: usize,
    ) -> anyhow::Result<HttpProxyServer> {
        let reload_sender = crate::serve::new_reload_notify_channel();

        let mut tls_accept_timeout = Duration::from_secs(10);
        let tls_acceptor = if let Some(tls_config_builder) = &config.server_tls_config {
            let tls_server_config = tls_config_builder
                .build_with_alpn_protocols(
                    Some(vec![AlpnProtocol::Http11, AlpnProtocol::Http10]),
                    tls_rolling_ticketer.clone(),
                )
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
            .map(|builder| builder.build());

        let dst_host_filter = config
            .dst_host_filter
            .as_ref()
            .map(|builder| Arc::new(builder.build()));

        let task_logger = config.get_task_logger();
        let idle_wheel = IdleWheel::spawn(config.task_idle_check_duration);

        // always update extra metrics tags
        server_stats.set_extra_tags(config.extra_metrics_tags.clone());

        let escaper = Arc::new(crate::escape::get_or_insert_default(config.escaper()));
        let user_group = config.get_user_group();
        let audit_handle = config.get_audit_handle()?;

        let server = HttpProxyServer {
            config,
            server_stats,
            listen_stats,
            tls_rolling_ticketer,
            tls_acceptor,
            tls_accept_timeout,
            tls_client_config: Arc::new(tls_client_config),
            ingress_net_filter,
            dst_host_filter,
            reload_sender,
            task_logger,
            escaper: ArcSwap::new(escaper),
            user_group: ArcSwapOption::new(user_group),
            audit_handle: ArcSwapOption::new(audit_handle),
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            idle_wheel,
            reload_version: version,
        };

        Ok(server)
    }

    pub(crate) fn prepare_initial(config: HttpProxyServerConfig) -> anyhow::Result<ArcServer> {
        let config = Arc::new(config);
        let server_stats = Arc::new(HttpProxyServerStats::new(config.name()));
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
            HttpProxyServer::new(config, server_stats, listen_stats, tls_rolling_ticketer, 1)?;
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<HttpProxyServer> {
        if let AnyServerConfig::HttpProxy(config) = config {
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

            let server = HttpProxyServer::new(
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

    fn get_common_task_context(&self, cc_info: ClientConnectionInfo) -> Arc<CommonTaskContext> {
        Arc::new(CommonTaskContext {
            server_config: self.config.clone(),
            server_stats: self.server_stats.clone(),
            server_quit_policy: self.quit_policy.clone(),
            idle_wheel: self.idle_wheel.clone(),
            escaper: self.escaper.load().as_ref().clone(),
            cc_info,
            tls_client_config: self.tls_client_config.clone(),
            task_logger: self.task_logger.clone(),
            dst_host_filter: self.dst_host_filter.clone(),
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

    fn audit_context(&self) -> AuditContext {
        AuditContext::new(self.audit_handle.load_full())
    }

    async fn spawn_stream_task<T>(&self, stream: T, cc_info: ClientConnectionInfo)
    where
        T: AsyncStream,
        T::R: AsyncRead + Send + Sync + Unpin + 'static,
        T::W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let ctx = self.get_common_task_context(cc_info);
        let pipeline_stats = Arc::new(HttpProxyPipelineStats::default());
        let (task_sender, task_receiver) = mpsc::channel(ctx.server_config.pipeline_size.get());

        // NOTE tls underlying traffic is not counted in (server/task/user) stats

        let (clt_r, clt_w) = stream.into_split();
        let r_task = HttpProxyPipelineReaderTask::new(&ctx, task_sender, clt_r, &pipeline_stats);
        let w_task = HttpProxyPipelineWriterTask::new(
            &ctx,
            self.audit_context(),
            self.user_group.load_full(),
            task_receiver,
            clt_w,
            &pipeline_stats,
        );

        tokio::spawn(r_task.into_running());
        w_task.into_running().await
    }

    #[cfg(feature = "quic")]
    fn spawn_quic_stream_task(
        &self,
        send_stream: quinn::SendStream,
        recv_stream: quinn::RecvStream,
        cc_info: ClientConnectionInfo,
    ) {
        let ctx = self.get_common_task_context(cc_info);
        let pipeline_stats = Arc::new(HttpProxyPipelineStats::default());
        let (task_sender, task_receiver) = mpsc::channel(ctx.server_config.pipeline_size.get());

        let r_task =
            HttpProxyPipelineReaderTask::new(&ctx, task_sender, recv_stream, &pipeline_stats);
        tokio::spawn(r_task.into_running());

        let w_task = HttpProxyPipelineWriterTask::new(
            &ctx,
            self.audit_context(),
            self.user_group.load_full(),
            task_receiver,
            send_stream,
            &pipeline_stats,
        );
        tokio::spawn(w_task.into_running());
    }
}

impl ServerInternal for HttpProxyServer {
    fn _clone_config(&self) -> AnyServerConfig {
        AnyServerConfig::HttpProxy(Box::new(self.config.as_ref().clone()))
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

    fn _update_user_group_in_place(&self) {
        self.user_group.store(self.config.get_user_group());
    }

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

impl BaseServer for HttpProxyServer {
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

#[async_trait]
impl AcceptTcpServer for HttpProxyServer {
    async fn run_tcp_task(&self, stream: TcpStream, cc_info: ClientConnectionInfo) {
        let client_addr = cc_info.client_addr();
        self.server_stats.add_conn(client_addr);
        if self.drop_early(client_addr) {
            return;
        }

        if let Some(tls_acceptor) = &self.tls_acceptor {
            match tokio::time::timeout(self.tls_accept_timeout, tls_acceptor.accept(stream)).await {
                Ok(Ok(tls_stream)) => {
                    if tls_stream.get_ref().1.session_reused() {
                        // Quick ACK is needed with session resumption
                        cc_info.tcp_sock_try_quick_ack();
                    }
                    self.spawn_stream_task(tls_stream, cc_info).await
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
        } else {
            self.spawn_stream_task(stream, cc_info).await;
        }
    }
}

#[async_trait]
impl AcceptQuicServer for HttpProxyServer {
    #[cfg(feature = "quic")]
    async fn run_quic_task(&self, connection: Connection, cc_info: ClientConnectionInfo) {
        let client_addr = cc_info.client_addr();
        self.server_stats.add_conn(client_addr);
        if self.drop_early(client_addr) {
            return;
        }

        loop {
            // TODO update ctx and quit gracefully
            match connection.accept_bi().await {
                Ok((send_stream, recv_stream)) => {
                    self.spawn_quic_stream_task(send_stream, recv_stream, cc_info.clone())
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
impl Server for HttpProxyServer {
    fn escaper(&self) -> &NodeName {
        self.config.escaper()
    }

    fn user_group(&self) -> &NodeName {
        self.config.user_group()
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

        self.spawn_stream_task(stream, cc_info).await;
    }

    async fn run_openssl_task(&self, stream: SslStream<TcpStream>, cc_info: ClientConnectionInfo) {
        let client_addr = cc_info.client_addr();
        self.server_stats.add_conn(client_addr);
        if self.drop_early(client_addr) {
            return;
        }

        self.spawn_stream_task(stream, cc_info).await;
    }
}
