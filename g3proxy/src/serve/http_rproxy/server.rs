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
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use arc_swap::{ArcSwap, ArcSwapOption};
use async_trait::async_trait;
use log::debug;
#[cfg(feature = "quic")]
use quinn::Connection;
use slog::Logger;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc};
use tokio_rustls::server::TlsStream;
use tokio_rustls::LazyConfigAcceptor;

use g3_daemon::listen::{AcceptQuicServer, AcceptTcpServer, ListenStats, ListenTcpRuntime};
use g3_daemon::server::{BaseServer, ClientConnectionInfo, ServerReloadCommand};
use g3_io_ext::{AsyncStream, IdleWheel};
use g3_openssl::SslStream;
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::metrics::NodeName;
use g3_types::net::{
    AlpnProtocol, OpensslTicketKey, RollingTicketer, RustlsServerConfig, RustlsServerConnectionExt,
    UpstreamAddr,
};
use g3_types::route::HostMatch;

use super::task::{
    CommonTaskContext, HttpRProxyPipelineReaderTask, HttpRProxyPipelineStats,
    HttpRProxyPipelineWriterTask,
};
use super::{HttpHost, HttpRProxyServerStats};
use crate::auth::UserGroup;
use crate::config::server::http_rproxy::HttpRProxyServerConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::escape::ArcEscaper;
use crate::serve::{
    ArcServer, ArcServerStats, Server, ServerInternal, ServerQuitPolicy, ServerStats, WrapArcServer,
};

pub(crate) struct HttpRProxyServer {
    config: Arc<HttpRProxyServerConfig>,
    server_stats: Arc<HttpRProxyServerStats>,
    listen_stats: Arc<ListenStats>,
    tls_rolling_ticketer: Option<Arc<RollingTicketer<OpensslTicketKey>>>,
    global_tls_server: Option<RustlsServerConfig>,
    ingress_net_filter: Option<AclNetworkRule>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
    task_logger: Logger,
    hosts: HostMatch<Arc<HttpHost>>,

    escaper: ArcSwap<ArcEscaper>,
    user_group: ArcSwapOption<UserGroup>,
    quit_policy: Arc<ServerQuitPolicy>,
    idle_wheel: Arc<IdleWheel>,
    reload_version: usize,
}

impl HttpRProxyServer {
    fn new(
        config: Arc<HttpRProxyServerConfig>,
        server_stats: Arc<HttpRProxyServerStats>,
        listen_stats: Arc<ListenStats>,
        hosts: HostMatch<Arc<HttpHost>>,
        tls_rolling_ticketer: Option<Arc<RollingTicketer<OpensslTicketKey>>>,
        version: usize,
    ) -> anyhow::Result<Self> {
        let reload_sender = crate::serve::new_reload_notify_channel();

        let global_tls_server = match &config.global_tls_server {
            Some(builder) => {
                let config = builder
                    .build_with_alpn_protocols(
                        Some(vec![AlpnProtocol::Http11, AlpnProtocol::Http10]),
                        tls_rolling_ticketer.clone(),
                    )
                    .context("failed to build global tls server config")?;
                Some(config)
            }
            None => None,
        };

        let ingress_net_filter = config
            .ingress_net_filter
            .as_ref()
            .map(|builder| builder.build());

        let task_logger = config.get_task_logger();
        let idle_wheel = IdleWheel::spawn(config.task_idle_check_duration);

        // always update extra metrics tags
        server_stats.set_extra_tags(config.extra_metrics_tags.clone());

        let escaper = Arc::new(crate::escape::get_or_insert_default(config.escaper()));
        let user_group = config.get_user_group();

        let server = HttpRProxyServer {
            config,
            server_stats,
            listen_stats,
            tls_rolling_ticketer,
            global_tls_server,
            ingress_net_filter,
            reload_sender,
            task_logger,
            hosts,
            escaper: ArcSwap::new(escaper),
            user_group: ArcSwapOption::new(user_group),
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            idle_wheel,
            reload_version: version,
        };

        Ok(server)
    }

    pub(crate) fn prepare_initial(config: HttpRProxyServerConfig) -> anyhow::Result<ArcServer> {
        let config = Arc::new(config);
        let server_stats = Arc::new(HttpRProxyServerStats::new(config.name()));
        let listen_stats = Arc::new(ListenStats::new(config.name()));

        let tls_rolling_ticketer = if let Some(c) = &config.tls_ticketer {
            let ticketer = c
                .build_and_spawn_updater()
                .context("failed to create tls rolling ticketer")?;
            Some(ticketer)
        } else {
            None
        };
        let hosts = config
            .hosts
            .try_build_arc(|c| HttpHost::try_build(c, tls_rolling_ticketer.clone()))?;

        let server = HttpRProxyServer::new(
            config,
            server_stats,
            listen_stats,
            hosts,
            tls_rolling_ticketer,
            1,
        )?;
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<HttpRProxyServer> {
        if let AnyServerConfig::HttpRProxy(config) = config {
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
            // TODO do update if host has runtime state
            let hosts = config
                .hosts
                .try_build_arc(|c| HttpHost::try_build(c, tls_rolling_ticketer.clone()))?;

            let server = HttpRProxyServer::new(
                config,
                server_stats,
                listen_stats,
                hosts,
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
            task_logger: self.task_logger.clone(),
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

    async fn spawn_stream_task<T>(&self, stream: T, cc_info: ClientConnectionInfo)
    where
        T: AsyncStream,
        T::R: AsyncRead + Send + Sync + Unpin + 'static,
        T::W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let ctx = self.get_common_task_context(cc_info);
        let pipeline_stats = Arc::new(HttpRProxyPipelineStats::default());
        let (task_sender, task_receiver) = mpsc::channel(ctx.server_config.pipeline_size.get());

        // NOTE tls underlying traffic is not counted in (server/task/user) stats

        let (clt_r, clt_w) = stream.into_split();
        let r_task = HttpRProxyPipelineReaderTask::new(&ctx, task_sender, clt_r, &pipeline_stats);
        let w_task = HttpRProxyPipelineWriterTask::new(
            &ctx,
            self.user_group.load_full(),
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

impl BaseServer for HttpRProxyServer {
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
impl AcceptTcpServer for HttpRProxyServer {
    async fn run_tcp_task(&self, stream: TcpStream, cc_info: ClientConnectionInfo) {
        let client_addr = cc_info.client_addr();
        self.server_stats.add_conn(client_addr);
        if self.drop_early(client_addr) {
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
                                    if stream.get_ref().1.session_reused() {
                                        // Quick ACK is needed with session resumption
                                        cc_info.tcp_sock_try_quick_ack();
                                    }
                                    self.spawn_stream_task(stream, cc_info).await
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
                        None => {
                            // No tls server config found
                            self.listen_stats.add_failed();
                            debug!(
                                "{} - {} tls error: no matched server config found",
                                cc_info.sock_local_addr(),
                                cc_info.sock_peer_addr()
                            );
                        }
                    }
                }
                Ok(Err(e)) => {
                    self.listen_stats.add_failed();
                    debug!(
                        "{} - {} tls client hello error: {e:?}",
                        cc_info.sock_local_addr(),
                        cc_info.sock_peer_addr()
                    );
                    // TODO record tls failure and add some sec policy
                }
                Err(_) => {
                    self.listen_stats.add_timeout();
                    debug!(
                        "{} - {} tls client hello timeout",
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
impl AcceptQuicServer for HttpRProxyServer {
    #[cfg(feature = "quic")]
    async fn run_quic_task(&self, _connection: Connection, _cc_info: ClientConnectionInfo) {}
}

#[async_trait]
impl Server for HttpRProxyServer {
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
