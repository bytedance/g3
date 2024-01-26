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
use anyhow::anyhow;
use async_trait::async_trait;
use log::warn;
use openssl::ex_data::Index;
#[cfg(feature = "vendored-tongsuo")]
use openssl::ssl::SslVersion;
use openssl::ssl::{Ssl, SslContext};
#[cfg(feature = "quic")]
use quinn::Connection;
use slog::Logger;
use tokio::net::TcpStream;
use tokio::sync::broadcast;

use g3_daemon::listen::{AcceptQuicServer, AcceptTcpServer, ListenStats, ListenTcpRuntime};
use g3_daemon::server::{BaseServer, ClientConnectionInfo, ServerReloadCommand};
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::metrics::MetricsName;
use g3_types::net::Host;
use g3_types::route::HostMatch;

use super::{CommonTaskContext, OpensslAcceptTask, OpensslHost, OpensslProxyServerStats};
use crate::config::server::openssl_proxy::OpensslProxyServerConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::serve::{
    ArcServer, ArcServerStats, Server, ServerInternal, ServerQuitPolicy, ServerStats, WrapArcServer,
};

pub(crate) struct OpensslProxyServer {
    config: Arc<OpensslProxyServerConfig>,
    server_stats: Arc<OpensslProxyServerStats>,
    listen_stats: Arc<ListenStats>,
    ingress_net_filter: Option<AclNetworkRule>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
    task_logger: Logger,
    hosts: Arc<HostMatch<Arc<OpensslHost>>>,
    #[cfg(feature = "vendored-tongsuo")]
    client_hello_version_index: Index<Ssl, SslVersion>,
    host_name_index: Index<Ssl, Host>,
    lazy_ssl_context: SslContext,

    quit_policy: Arc<ServerQuitPolicy>,
    reload_version: usize,
}

impl OpensslProxyServer {
    fn new(
        config: Arc<OpensslProxyServerConfig>,
        server_stats: Arc<OpensslProxyServerStats>,
        listen_stats: Arc<ListenStats>,
        hosts: Arc<HostMatch<Arc<OpensslHost>>>,
        version: usize,
    ) -> anyhow::Result<Self> {
        let reload_sender = crate::serve::new_reload_notify_channel();

        let host_name_index =
            Ssl::new_ex_index().map_err(|e| anyhow!("failed to create ex index: {e}"))?;
        #[cfg(feature = "vendored-tongsuo")]
        let client_hello_version_index =
            Ssl::new_ex_index().map_err(|e| anyhow!("failed to create ex index: {e}"))?;
        #[cfg(feature = "vendored-tongsuo")]
        let lazy_ssl_context =
            super::host::build_lazy_ssl_context(client_hello_version_index, host_name_index)?;
        #[cfg(not(feature = "vendored-tongsuo"))]
        let lazy_ssl_context = super::host::build_lazy_ssl_context(host_name_index)?;

        let ingress_net_filter = config
            .ingress_net_filter
            .as_ref()
            .map(|builder| builder.build());

        let task_logger = config.get_task_logger();

        // always update extra metrics tags
        server_stats.set_extra_tags(config.extra_metrics_tags.clone());

        Ok(OpensslProxyServer {
            config,
            server_stats,
            listen_stats,
            ingress_net_filter,
            reload_sender,
            task_logger,
            hosts,
            #[cfg(feature = "vendored-tongsuo")]
            client_hello_version_index,
            host_name_index,
            lazy_ssl_context,
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            reload_version: version,
        })
    }

    pub(crate) fn prepare_initial(config: OpensslProxyServerConfig) -> anyhow::Result<ArcServer> {
        let config = Arc::new(config);
        let server_stats = Arc::new(OpensslProxyServerStats::new(config.name()));
        let listen_stats = Arc::new(ListenStats::new(config.name()));

        let hosts = config.hosts.try_build_arc(OpensslHost::try_build)?;

        let server =
            OpensslProxyServer::new(config, server_stats, listen_stats, Arc::new(hosts), 1)?;
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<OpensslProxyServer> {
        if let AnyServerConfig::OpensslProxy(config) = config {
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
                    OpensslHost::try_build(&conf)?
                };
                new_hosts_map.insert(name, Arc::new(host));
            }

            let hosts = config.hosts.build_from(new_hosts_map);

            OpensslProxyServer::new(
                config,
                server_stats,
                listen_stats,
                Arc::new(hosts),
                self.reload_version + 1,
            )
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
        let ssl = match Ssl::new(&self.lazy_ssl_context) {
            Ok(v) => v,
            Err(e) => {
                warn!("failed to build ssl context when accepting connections: {e}");
                return;
            }
        };

        let ctx = CommonTaskContext {
            server_config: Arc::clone(&self.config),
            server_stats: Arc::clone(&self.server_stats),
            server_quit_policy: Arc::clone(&self.quit_policy),
            cc_info,
            task_logger: self.task_logger.clone(),

            #[cfg(feature = "vendored-tongsuo")]
            client_hello_version_index: self.client_hello_version_index,
            host_name_index: self.host_name_index,
        };

        if self.config.spawn_task_unconstrained {
            tokio::task::unconstrained(
                OpensslAcceptTask::new(ctx, self.hosts.clone()).into_running(stream, ssl),
            )
            .await
        } else {
            OpensslAcceptTask::new(ctx, self.hosts.clone())
                .into_running(stream, ssl)
                .await;
        }
    }
}

impl ServerInternal for OpensslProxyServer {
    fn _clone_config(&self) -> AnyServerConfig {
        AnyServerConfig::OpensslProxy(self.config.as_ref().clone())
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
        let runtime =
            ListenTcpRuntime::new(WrapArcServer(server.clone()), server.get_listen_stats());
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

impl BaseServer for OpensslProxyServer {
    #[inline]
    fn name(&self) -> &MetricsName {
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
impl AcceptTcpServer for OpensslProxyServer {
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
impl AcceptQuicServer for OpensslProxyServer {
    #[cfg(feature = "quic")]
    async fn run_quic_task(&self, _connection: Connection, _cc_info: ClientConnectionInfo) {}
}

#[async_trait]
impl Server for OpensslProxyServer {
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

    fn update_backend(&self, name: &MetricsName) {
        let host_map = self.hosts.get_all_values();
        for host in host_map.values() {
            if host.use_backend(name) {
                host.update_backends();
            }
        }
    }
}
