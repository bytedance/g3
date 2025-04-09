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
use arc_swap::ArcSwap;
use async_trait::async_trait;
#[cfg(feature = "quic")]
use quinn::Connection;
use slog::Logger;
#[cfg(feature = "quic")]
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::broadcast;

use g3_daemon::listen::{AcceptQuicServer, AcceptTcpServer, ListenStats};
use g3_daemon::server::{BaseServer, ClientConnectionInfo};
use g3_io_ext::IdleWheel;
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::metrics::NodeName;

use super::{CommonTaskContext, KeylessForwardTask, KeylessProxyServerStats};
use crate::backend::ArcBackend;
use crate::config::server::keyless_proxy::KeylessProxyServerConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::serve::{
    ArcServer, ArcServerStats, Server, ServerInternal, ServerQuitPolicy, ServerRegistry,
    ServerReloadCommand, ServerStats,
};

pub(crate) struct KeylessProxyServer {
    config: Arc<KeylessProxyServerConfig>,
    server_stats: Arc<KeylessProxyServerStats>,
    listen_stats: Arc<ListenStats>,
    ingress_net_filter: Option<Arc<AclNetworkRule>>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
    task_logger: Logger,

    backend_selector: Arc<ArcSwap<ArcBackend>>,
    quit_policy: Arc<ServerQuitPolicy>,
    idle_wheel: Arc<IdleWheel>,
    reload_version: usize,
}

impl KeylessProxyServer {
    fn new(
        config: Arc<KeylessProxyServerConfig>,
        server_stats: Arc<KeylessProxyServerStats>,
        listen_stats: Arc<ListenStats>,
        version: usize,
    ) -> anyhow::Result<Self> {
        let reload_sender = crate::serve::new_reload_notify_channel();

        let ingress_net_filter = config
            .ingress_net_filter
            .as_ref()
            .map(|builder| Arc::new(builder.build()));

        let backend = crate::backend::get_or_insert_default(&config.backend);

        let task_logger = config.get_task_logger();
        let idle_wheel = IdleWheel::spawn(config.task_idle_check_duration);

        // always update extra metrics tags
        server_stats.set_extra_tags(config.extra_metrics_tags.clone());

        Ok(KeylessProxyServer {
            config,
            server_stats,
            listen_stats,
            ingress_net_filter,
            reload_sender,
            task_logger,
            backend_selector: Arc::new(ArcSwap::from_pointee(backend)),
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            idle_wheel,
            reload_version: version,
        })
    }

    pub(crate) fn prepare_initial(config: KeylessProxyServerConfig) -> anyhow::Result<ArcServer> {
        let config = Arc::new(config);
        let server_stats = Arc::new(KeylessProxyServerStats::new(config.name()));
        let listen_stats = Arc::new(ListenStats::new(config.name()));

        // TODO crate::stat::metrics::server::keyless::push_stats(server_stats.clone());
        let server = KeylessProxyServer::new(config, server_stats, listen_stats, 1)?;
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<KeylessProxyServer> {
        if let AnyServerConfig::KeylessProxy(config) = config {
            let config = Arc::new(config);
            let server_stats = Arc::clone(&self.server_stats);
            let listen_stats = Arc::clone(&self.listen_stats);

            KeylessProxyServer::new(config, server_stats, listen_stats, self.reload_version + 1)
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
            server_config: self.config.clone(),
            server_stats: self.server_stats.clone(),
            server_quit_policy: self.quit_policy.clone(),
            idle_wheel: self.idle_wheel.clone(),
            cc_info,
            task_logger: self.task_logger.clone(),
            backend_selector: self.backend_selector.clone(),
        };

        let (clt_r, clt_w) = stream.into_split();
        let task = KeylessForwardTask::new(ctx);
        if self.config.spawn_task_unconstrained {
            tokio::task::unconstrained(task.into_running(clt_r, clt_w)).await
        } else {
            task.into_running(clt_r, clt_w).await
        }
    }

    #[cfg(feature = "quic")]
    fn spawn_task<R, W>(&self, clt_r: R, clt_w: W, cc_info: ClientConnectionInfo)
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let ctx = CommonTaskContext {
            server_config: self.config.clone(),
            server_stats: self.server_stats.clone(),
            server_quit_policy: self.quit_policy.clone(),
            idle_wheel: self.idle_wheel.clone(),
            cc_info,
            task_logger: self.task_logger.clone(),
            backend_selector: self.backend_selector.clone(),
        };

        let task = KeylessForwardTask::new(ctx);
        if self.config.spawn_task_unconstrained {
            tokio::spawn(async move {
                tokio::task::unconstrained(task.into_running(clt_r, clt_w)).await
            });
        } else {
            tokio::spawn(async move { task.into_running(clt_r, clt_w).await });
        }
    }
}

impl ServerInternal for KeylessProxyServer {
    fn _clone_config(&self) -> AnyServerConfig {
        AnyServerConfig::KeylessProxy(self.config.as_ref().clone())
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
    ) -> anyhow::Result<ArcServer> {
        let mut server = self.prepare_reload(config)?;
        server.reload_sender = self.reload_sender.clone();
        Ok(Arc::new(server))
    }

    fn _reload_with_new_notifier(
        &self,
        config: AnyServerConfig,
        _registry: &mut ServerRegistry,
    ) -> anyhow::Result<ArcServer> {
        let server = self.prepare_reload(config)?;
        Ok(Arc::new(server))
    }

    fn _start_runtime(&self, _server: &ArcServer) -> anyhow::Result<()> {
        self.server_stats.set_online();
        Ok(())
    }

    fn _abort_runtime(&self) {
        self.server_stats.set_offline();
    }
}

impl BaseServer for KeylessProxyServer {
    #[inline]
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    fn server_type(&self) -> &'static str {
        self.config.server_type()
    }

    #[inline]
    fn version(&self) -> usize {
        self.reload_version
    }
}

#[async_trait]
impl AcceptTcpServer for KeylessProxyServer {
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
impl AcceptQuicServer for KeylessProxyServer {
    #[cfg(feature = "quic")]
    async fn run_quic_task(&self, connection: Connection, cc_info: ClientConnectionInfo) {
        let client_addr = cc_info.client_addr();
        self.server_stats.add_conn(client_addr);
        if self.drop_early(client_addr) {
            return;
        }

        while let Ok((send_stream, recv_stream)) = connection.accept_bi().await {
            self.spawn_task(recv_stream, send_stream, cc_info.clone());
        }
    }
}

#[async_trait]
impl Server for KeylessProxyServer {
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
        if self.config.backend.eq(name) {
            let backend = crate::backend::get_or_insert_default(name);
            self.backend_selector.store(Arc::new(backend));
        }
    }
}
