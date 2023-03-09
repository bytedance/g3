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
use std::os::fd::AsRawFd;
use std::sync::Arc;

use ahash::AHashMap;
use anyhow::anyhow;
use async_trait::async_trait;
use slog::Logger;
use tokio::net::TcpStream;
use tokio::sync::broadcast;

use g3_daemon::listen::ListenStats;
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::route::HostMatch;

use super::{CommonTaskContext, RustlsAcceptTask, RustlsHost, RustlsProxyServerStats};
use crate::config::server::rustls_proxy::RustlsProxyServerConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::serve::{
    ArcServer, ArcServerStats, OrdinaryTcpServerRuntime, Server, ServerInternal, ServerQuitPolicy,
    ServerReloadCommand, ServerRunContext, ServerStats,
};

pub(crate) struct RustlsProxyServer {
    config: Arc<RustlsProxyServerConfig>,
    server_stats: Arc<RustlsProxyServerStats>,
    listen_stats: Arc<ListenStats>,
    ingress_net_filter: Option<Arc<AclNetworkRule>>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
    task_logger: Logger,
    hosts: HostMatch<Arc<RustlsHost>>,

    quit_policy: Arc<ServerQuitPolicy>,
    reload_version: usize,
}

impl RustlsProxyServer {
    fn new(
        config: Arc<RustlsProxyServerConfig>,
        server_stats: Arc<RustlsProxyServerStats>,
        listen_stats: Arc<ListenStats>,
        hosts: HostMatch<Arc<RustlsHost>>,
        version: usize,
    ) -> Self {
        let (reload_sender, _reload_receiver) = crate::serve::new_reload_notify_channel();

        let ingress_net_filter = config
            .ingress_net_filter
            .as_ref()
            .map(|builder| Arc::new(builder.build()));

        let task_logger = config.get_task_logger();

        // always update extra metrics tags
        server_stats.set_extra_tags(config.extra_metrics_tags.clone());

        RustlsProxyServer {
            config,
            server_stats,
            listen_stats,
            ingress_net_filter,
            reload_sender,
            task_logger,
            hosts,
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            reload_version: version,
        }
    }

    pub(crate) fn prepare_initial(config: AnyServerConfig) -> anyhow::Result<ArcServer> {
        if let AnyServerConfig::RustlsProxy(config) = config {
            let config = Arc::new(config);
            let server_stats = Arc::new(RustlsProxyServerStats::new(config.name()));
            let listen_stats = Arc::new(ListenStats::new(config.name()));

            let hosts = (&config.hosts).try_into()?;

            let server = RustlsProxyServer::new(config, server_stats, listen_stats, hosts, 1);
            Ok(Arc::new(server))
        } else {
            Err(anyhow!("invalid config type for DummyClose server"))
        }
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<RustlsProxyServer> {
        if let AnyServerConfig::RustlsProxy(config) = config {
            let config = Arc::new(config);
            let server_stats = Arc::clone(&self.server_stats);
            let listen_stats = Arc::clone(&self.listen_stats);

            let old_hosts_map = self.hosts.get_all_values();
            let new_conf_map = config.hosts.get_all_values();
            let mut new_hosts_map = AHashMap::with_capacity(new_conf_map.len());
            for (name, conf) in new_conf_map {
                let host = if let Some(old_host) = old_hosts_map.get(&name) {
                    old_host.new_for_reload(conf)?
                } else {
                    RustlsHost::build_new(conf)?
                };
                new_hosts_map.insert(name, Arc::new(host));
            }
            let hosts = config.hosts.build_from(new_hosts_map);

            let server = RustlsProxyServer::new(
                config,
                server_stats,
                listen_stats,
                hosts,
                self.reload_version + 1,
            );
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
        _run_ctx: ServerRunContext,
    ) {
        let ctx = CommonTaskContext {
            server_config: Arc::clone(&self.config),
            server_stats: Arc::clone(&self.server_stats),
            server_quit_policy: Arc::clone(&self.quit_policy),
            server_addr: local_addr,
            client_addr: peer_addr,
            task_logger: self.task_logger.clone(),
            tcp_client_socket: stream.as_raw_fd(),
        };

        if self.config.spawn_task_unconstrained {
            tokio::task::unconstrained(RustlsAcceptTask::new(ctx).into_running(stream, &self.hosts))
                .await
        } else {
            RustlsAcceptTask::new(ctx)
                .into_running(stream, &self.hosts)
                .await;
        }
    }
}

impl ServerInternal for RustlsProxyServer {
    fn _clone_config(&self) -> AnyServerConfig {
        AnyServerConfig::RustlsProxy(self.config.as_ref().clone())
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
impl Server for RustlsProxyServer {
    fn name(&self) -> &str {
        self.config.name()
    }

    fn version(&self) -> usize {
        self.reload_version
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
}
