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

use anyhow::anyhow;
use arc_swap::{ArcSwap, ArcSwapOption};
use async_trait::async_trait;
#[cfg(feature = "quic")]
use quinn::Connection;
use slog::Logger;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_openssl::SslStream;
use tokio_rustls::server::TlsStream;

use g3_daemon::listen::ListenStats;
use g3_daemon::server::{ClientConnectionInfo, ServerReloadCommand};
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::acl_set::AclDstHostRuleSet;
use g3_types::metrics::MetricsName;

use super::task::{CommonTaskContext, SocksProxyNegotiationTask};
use super::SocksProxyServerStats;
use crate::audit::AuditHandle;
use crate::auth::UserGroup;
use crate::config::server::socks_proxy::SocksProxyServerConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::escape::ArcEscaper;
use crate::serve::{
    ArcServer, ArcServerStats, ListenTcpRuntime, Server, ServerInternal, ServerQuitPolicy,
    ServerStats,
};

pub(crate) struct SocksProxyServer {
    config: Arc<SocksProxyServerConfig>,
    server_stats: Arc<SocksProxyServerStats>,
    listen_stats: Arc<ListenStats>,
    ingress_net_filter: Option<Arc<AclNetworkRule>>,
    dst_host_filter: Option<Arc<AclDstHostRuleSet>>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
    task_logger: Logger,

    escaper: ArcSwap<ArcEscaper>,
    user_group: ArcSwapOption<UserGroup>,
    audit_handle: ArcSwapOption<AuditHandle>,
    quit_policy: Arc<ServerQuitPolicy>,
    reload_version: usize,
}

impl SocksProxyServer {
    fn new(
        config: Arc<SocksProxyServerConfig>,
        server_stats: Arc<SocksProxyServerStats>,
        listen_stats: Arc<ListenStats>,
        version: usize,
    ) -> anyhow::Result<SocksProxyServer> {
        let reload_sender = crate::serve::new_reload_notify_channel();

        let ingress_net_filter = config
            .ingress_net_filter
            .as_ref()
            .map(|builder| Arc::new(builder.build()));

        let dst_host_filter = config
            .dst_host_filter
            .as_ref()
            .map(|builder| Arc::new(builder.build()));

        let task_logger = config.get_task_logger();

        server_stats.set_extra_tags(config.extra_metrics_tags.clone());

        let escaper = Arc::new(crate::escape::get_or_insert_default(config.escaper()));
        let user_group = config.get_user_group();
        let audit_handle = config.get_audit_handle()?;

        let server = SocksProxyServer {
            config,
            server_stats,
            listen_stats,
            ingress_net_filter,
            dst_host_filter,
            reload_sender,
            task_logger,
            escaper: ArcSwap::new(escaper),
            user_group: ArcSwapOption::new(user_group),
            audit_handle: ArcSwapOption::new(audit_handle),
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            reload_version: version,
        };

        Ok(server)
    }

    pub(crate) fn prepare_initial(config: SocksProxyServerConfig) -> anyhow::Result<ArcServer> {
        let config = Arc::new(config);
        let server_stats = Arc::new(SocksProxyServerStats::new(config.name()));
        let listen_stats = Arc::new(ListenStats::new(config.name()));

        let server = SocksProxyServer::new(config, server_stats, listen_stats, 1)?;
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<SocksProxyServer> {
        if let AnyServerConfig::SocksProxy(config) = config {
            let config = Arc::new(*config);
            let server_stats = Arc::clone(&self.server_stats);
            let listen_stats = Arc::clone(&self.listen_stats);

            let server =
                SocksProxyServer::new(config, server_stats, listen_stats, self.reload_version + 1)?;
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

    async fn run_task(&self, stream: TcpStream, cc_info: ClientConnectionInfo) {
        let ctx = CommonTaskContext {
            server_config: Arc::clone(&self.config),
            server_stats: Arc::clone(&self.server_stats),
            server_quit_policy: Arc::clone(&self.quit_policy),
            escaper: self.escaper.load().as_ref().clone(),
            audit_handle: self.audit_handle.load_full(),
            ingress_net_filter: self.ingress_net_filter.clone(),
            dst_host_filter: self.dst_host_filter.clone(),
            cc_info,
            task_logger: self.task_logger.clone(),
        };
        SocksProxyNegotiationTask::new(ctx, self.user_group.load_full())
            .into_running(stream)
            .await;
    }
}

impl ServerInternal for SocksProxyServer {
    fn _clone_config(&self) -> AnyServerConfig {
        AnyServerConfig::SocksProxy(Box::new(self.config.as_ref().clone()))
    }

    fn _update_config_in_place(&self, _flags: u64, _config: AnyServerConfig) -> anyhow::Result<()> {
        Ok(())
    }

    fn _depend_on_server(&self, _name: &MetricsName) -> bool {
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
        let runtime = ListenTcpRuntime::new(server, &*self.config);
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

#[async_trait]
impl Server for SocksProxyServer {
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
        Some(Arc::clone(&self.server_stats) as _)
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

    async fn run_tcp_task(&self, stream: TcpStream, cc_info: ClientConnectionInfo) {
        let client_addr = cc_info.client_addr();
        self.server_stats.add_conn(client_addr);
        if self.drop_early(client_addr) {
            return;
        }

        self.run_task(stream, cc_info).await
    }

    async fn run_rustls_task(&self, _stream: TlsStream<TcpStream>, cc_info: ClientConnectionInfo) {
        self.server_stats.add_conn(cc_info.client_addr());
        self.listen_stats.add_dropped();
    }

    async fn run_openssl_task(&self, _stream: SslStream<TcpStream>, cc_info: ClientConnectionInfo) {
        self.server_stats.add_conn(cc_info.client_addr());
        self.listen_stats.add_dropped();
    }

    #[cfg(feature = "quic")]
    async fn run_quic_task(&self, _connection: Connection, _cc_info: ClientConnectionInfo) {}
}
