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

use ahash::AHashMap;
use anyhow::{Context, anyhow};
use async_trait::async_trait;
#[cfg(feature = "quic")]
use quinn::Connection;
use slog::Logger;
use tokio::net::TcpStream;
use tokio::sync::broadcast;

use g3_daemon::listen::{AcceptQuicServer, AcceptTcpServer, ListenStats, ListenTcpRuntime};
use g3_daemon::server::{BaseServer, ClientConnectionInfo, ServerReloadCommand};
use g3_io_ext::IdleWheel;
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::metrics::NodeName;
use g3_types::net::{OpensslTicketKey, RollingTicketer};
use g3_types::route::HostMatch;

use super::{CommonTaskContext, RustlsAcceptTask, RustlsHost};
use crate::config::server::rustls_proxy::RustlsProxyServerConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::module::stream::StreamServerStats;
use crate::serve::{
    ArcServer, ArcServerInternal, ArcServerStats, Server, ServerInternal, ServerQuitPolicy,
    ServerRegistry, ServerStats, WrapArcServer,
};

pub(crate) struct RustlsProxyServer {
    config: Arc<RustlsProxyServerConfig>,
    server_stats: Arc<StreamServerStats>,
    listen_stats: Arc<ListenStats>,
    ingress_net_filter: Option<AclNetworkRule>,
    tls_rolling_ticketer: Option<Arc<RollingTicketer<OpensslTicketKey>>>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
    task_logger: Option<Logger>,
    hosts: HostMatch<Arc<RustlsHost>>,

    quit_policy: Arc<ServerQuitPolicy>,
    idle_wheel: Arc<IdleWheel>,
    reload_version: usize,
}

impl RustlsProxyServer {
    fn new(
        config: Arc<RustlsProxyServerConfig>,
        server_stats: Arc<StreamServerStats>,
        listen_stats: Arc<ListenStats>,
        hosts: HostMatch<Arc<RustlsHost>>,
        tls_rolling_ticketer: Option<Arc<RollingTicketer<OpensslTicketKey>>>,
        version: usize,
    ) -> Self {
        let reload_sender = crate::serve::new_reload_notify_channel();

        let ingress_net_filter = config
            .ingress_net_filter
            .as_ref()
            .map(|builder| builder.build());

        let task_logger = config.get_task_logger();
        let idle_wheel = IdleWheel::spawn(config.task_idle_check_duration);

        // always update extra metrics tags
        server_stats.set_extra_tags(config.extra_metrics_tags.clone());

        RustlsProxyServer {
            config,
            server_stats,
            listen_stats,
            ingress_net_filter,
            tls_rolling_ticketer,
            reload_sender,
            task_logger,
            hosts,
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            idle_wheel,
            reload_version: version,
        }
    }

    pub(crate) fn prepare_initial(
        config: RustlsProxyServerConfig,
    ) -> anyhow::Result<ArcServerInternal> {
        let config = Arc::new(config);
        let server_stats = Arc::new(StreamServerStats::new(config.name()));
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
            .try_build_arc(|c| RustlsHost::try_build(c, tls_rolling_ticketer.clone()))?;

        let server = RustlsProxyServer::new(
            config,
            server_stats,
            listen_stats,
            hosts,
            tls_rolling_ticketer,
            1,
        );
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<RustlsProxyServer> {
        if let AnyServerConfig::RustlsProxy(config) = config {
            let config = Arc::new(config);
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

            let old_hosts_map = self.hosts.get_all_values();
            let new_conf_map = config.hosts.get_all_values();
            let mut new_hosts_map = AHashMap::with_capacity(new_conf_map.len());
            for (name, conf) in new_conf_map {
                let host = if let Some(old_host) = old_hosts_map.get(&name) {
                    old_host.new_for_reload(conf, tls_rolling_ticketer.clone())?
                } else {
                    RustlsHost::try_build(&conf, tls_rolling_ticketer.clone())?
                };
                new_hosts_map.insert(name, Arc::new(host));
            }
            let hosts = config.hosts.build_from(new_hosts_map);

            let server = RustlsProxyServer::new(
                config,
                server_stats,
                listen_stats,
                hosts,
                tls_rolling_ticketer,
                self.reload_version + 1,
            );
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

    async fn run_task(&self, stream: TcpStream, cc_info: ClientConnectionInfo) {
        let ctx = CommonTaskContext {
            server_config: self.config.clone(),
            server_stats: self.server_stats.clone(),
            server_quit_policy: self.quit_policy.clone(),
            idle_wheel: self.idle_wheel.clone(),
            cc_info,
            task_logger: self.task_logger.clone(),
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

    fn _depend_on_server(&self, _name: &NodeName) -> bool {
        false
    }

    fn _reload_config_notify_runtime(&self) {
        let cmd = ServerReloadCommand::ReloadVersion(self.reload_version);
        let _ = self.reload_sender.send(cmd);
    }

    fn _update_next_servers_in_place(&self) {}

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
        let listen_stats = server.get_listen_stats();
        let runtime = ListenTcpRuntime::new(WrapArcServer(server), listen_stats);
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

impl BaseServer for RustlsProxyServer {
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

#[async_trait]
impl AcceptTcpServer for RustlsProxyServer {
    async fn run_tcp_task(&self, stream: TcpStream, cc_info: ClientConnectionInfo) {
        let client_addr = cc_info.client_addr();
        self.server_stats.add_conn(client_addr);
        if self.drop_early(client_addr) {
            return;
        }

        self.run_task(stream, cc_info).await
    }
}

#[async_trait]
impl AcceptQuicServer for RustlsProxyServer {
    #[cfg(feature = "quic")]
    async fn run_quic_task(&self, _connection: Connection, _cc_info: ClientConnectionInfo) {}
}

#[async_trait]
impl Server for RustlsProxyServer {
    fn get_server_stats(&self) -> Option<ArcServerStats> {
        Some(self.server_stats.clone())
    }

    fn get_listen_stats(&self) -> Arc<ListenStats> {
        Arc::clone(&self.listen_stats)
    }

    fn alive_count(&self) -> i32 {
        self.server_stats.alive_count()
    }

    #[inline]
    fn quit_policy(&self) -> &Arc<ServerQuitPolicy> {
        &self.quit_policy
    }

    fn update_backend(&self, name: &NodeName) {
        let host_map = self.hosts.get_all_values();
        for host in host_map.values() {
            if host.use_backend(name) {
                host.update_backends();
            }
        }
    }
}
