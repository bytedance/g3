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
use std::os::unix::prelude::AsRawFd;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use slog::Logger;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_rustls::server::TlsStream;

use g3_daemon::listen::ListenStats;
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::acl_set::AclDstHostRuleSet;

use super::task::{CommonTaskContext, SocksProxyNegotiationTask};
use super::SocksProxyServerStats;
use crate::config::server::socks_proxy::SocksProxyServerConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::serve::{
    ArcServer, ArcServerStats, OrdinaryTcpServerRuntime, Server, ServerInternal, ServerQuitPolicy,
    ServerReloadCommand, ServerRunContext, ServerStats,
};

pub(crate) struct SocksProxyServer {
    config: Arc<SocksProxyServerConfig>,
    server_stats: Arc<SocksProxyServerStats>,
    listen_stats: Arc<ListenStats>,
    ingress_net_filter: Option<Arc<AclNetworkRule>>,
    dst_host_filter: Option<Arc<AclDstHostRuleSet>>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
    task_logger: Logger,

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
        let (reload_sender, _reload_receiver) = crate::serve::new_reload_notify_channel();

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

        let server = SocksProxyServer {
            config,
            server_stats,
            listen_stats,
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
        if let AnyServerConfig::SocksProxy(config) = config {
            let config = Arc::new(*config);
            let server_stats = Arc::new(SocksProxyServerStats::new(config.name()));
            let listen_stats = Arc::new(ListenStats::new(config.name()));

            let server = SocksProxyServer::new(config, server_stats, listen_stats, 1)?;
            Ok(Arc::new(server))
        } else {
            Err(anyhow!("invalid config type for SocksProxy server"))
        }
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

    async fn run_task(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
        local_addr: SocketAddr,
        run_ctx: ServerRunContext,
    ) {
        let ctx = CommonTaskContext {
            server_config: Arc::clone(&self.config),
            server_stats: Arc::clone(&self.server_stats),
            server_quit_policy: Arc::clone(&self.quit_policy),
            escaper: run_ctx.escaper,
            audit_handle: run_ctx.audit_handle,
            ingress_net_filter: self.ingress_net_filter.clone(),
            dst_host_filter: self.dst_host_filter.clone(),
            tcp_server_addr: local_addr,
            tcp_client_addr: peer_addr,
            task_logger: self.task_logger.clone(),
            tcp_client_socket: stream.as_raw_fd(),
            worker_id: run_ctx.worker_id,
        };
        SocksProxyNegotiationTask::new(ctx, run_ctx.user_group)
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
impl Server for SocksProxyServer {
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

    fn user_group(&self) -> String {
        self.config.user_group().to_string()
    }

    fn auditor(&self) -> String {
        self.config.auditor().to_string()
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
        self.run_task(stream, peer_addr, local_addr, ctx).await
    }

    async fn run_tls_task(
        &self,
        _stream: TlsStream<TcpStream>,
        peer_addr: SocketAddr,
        _local_addr: SocketAddr,
        _ctx: ServerRunContext,
    ) {
        self.server_stats.add_conn(peer_addr);
        self.listen_stats.add_dropped();
    }
}
