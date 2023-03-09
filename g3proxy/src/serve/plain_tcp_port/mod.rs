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
use tokio::net::TcpStream;
use tokio::runtime::Handle;
use tokio::sync::{broadcast, watch};
use tokio_rustls::server::TlsStream;

use g3_daemon::listen::ListenStats;
use g3_types::acl::{AclAction, AclNetworkRule};

use crate::config::server::plain_tcp_port::PlainTcpPortConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::serve::{
    ArcServer, AuxiliaryServerConfig, AuxiliaryTcpPortRuntime, Server, ServerInternal,
    ServerQuitPolicy, ServerReloadCommand, ServerRunContext,
};

#[derive(Clone)]
struct PlainTcpPortAuxConfig {
    config: Arc<PlainTcpPortConfig>,
    ingress_net_filter: Option<Arc<AclNetworkRule>>,
    listen_stats: Arc<ListenStats>,
}

impl AuxiliaryServerConfig for PlainTcpPortAuxConfig {
    fn next_server(&self) -> &str {
        &self.config.server
    }

    fn run_tcp_task(
        &self,
        rt_handle: Handle,
        next_server: ArcServer,
        stream: TcpStream,
        peer_addr: SocketAddr,
        local_addr: SocketAddr,
        ctx: ServerRunContext,
    ) {
        let ingress_net_filter = self.ingress_net_filter.clone();
        let listen_stats = self.listen_stats.clone();

        rt_handle.spawn(async move {
            if let Some(filter) = ingress_net_filter {
                let (_, action) = filter.check(peer_addr.ip());
                match action {
                    AclAction::Permit | AclAction::PermitAndLog => {}
                    AclAction::Forbid | AclAction::ForbidAndLog => {
                        listen_stats.add_dropped();
                        return;
                    }
                }
            }

            next_server
                .run_tcp_task(stream, peer_addr, local_addr, ctx)
                .await
        });
    }
}

pub(crate) struct PlainTcpPort {
    name: String,
    config: ArcSwap<PlainTcpPortConfig>,
    listen_stats: Arc<ListenStats>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,

    cfg_sender: watch::Sender<Option<PlainTcpPortAuxConfig>>,

    quit_policy: Arc<ServerQuitPolicy>,
    reload_version: usize,
}

impl PlainTcpPort {
    fn new(
        config: Arc<PlainTcpPortConfig>,
        listen_stats: Arc<ListenStats>,
        reload_version: usize,
    ) -> anyhow::Result<Self> {
        let (reload_sender, _reload_receiver) = crate::serve::new_reload_notify_channel();

        let ingress_net_filter = config
            .ingress_net_filter
            .as_ref()
            .map(|builder| Arc::new(builder.build()));

        let aux_config = PlainTcpPortAuxConfig {
            config: Arc::clone(&config),
            ingress_net_filter,
            listen_stats: Arc::clone(&listen_stats),
        };
        let (cfg_sender, _cfg_receiver) = watch::channel(Some(aux_config));

        Ok(PlainTcpPort {
            name: config.name().to_string(),
            config: ArcSwap::new(config),
            listen_stats,
            reload_sender,
            cfg_sender,
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            reload_version,
        })
    }

    pub(crate) fn prepare_initial(config: AnyServerConfig) -> anyhow::Result<ArcServer> {
        if let AnyServerConfig::PlainTcpPort(config) = config {
            let config = Arc::new(config);
            let listen_stats = Arc::new(ListenStats::new(config.name()));

            let server = PlainTcpPort::new(config, listen_stats, 1)?;
            Ok(Arc::new(server))
        } else {
            Err(anyhow!("invalid config type for PlainTcpPort server"))
        }
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<PlainTcpPort> {
        if let AnyServerConfig::PlainTcpPort(config) = config {
            let config = Arc::new(config);
            let listen_stats = Arc::clone(&self.listen_stats);

            PlainTcpPort::new(config, listen_stats, self.reload_version + 1)
        } else {
            let cur_config = self.config.load();
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                cur_config.server_type(),
                config.server_type()
            ))
        }
    }
}

impl ServerInternal for PlainTcpPort {
    fn _clone_config(&self) -> AnyServerConfig {
        let cur_config = self.config.load();
        AnyServerConfig::PlainTcpPort(cur_config.as_ref().clone())
    }

    fn _update_config_in_place(&self, _flags: u64, config: AnyServerConfig) -> anyhow::Result<()> {
        if let AnyServerConfig::PlainTcpPort(config) = config {
            let config = Arc::new(config);
            let ingress_net_filter = config
                .ingress_net_filter
                .as_ref()
                .map(|builder| Arc::new(builder.build()));
            let aux_config = PlainTcpPortAuxConfig {
                config: Arc::clone(&config),
                ingress_net_filter,
                listen_stats: Arc::clone(&self.listen_stats),
            };
            self.cfg_sender.send_replace(Some(aux_config));
            self.config.store(config);
            Ok(())
        } else {
            Err(anyhow!("invalid config type for PlainTcpPort server"))
        }
    }

    fn _get_reload_notifier(&self) -> broadcast::Receiver<ServerReloadCommand> {
        self.reload_sender.subscribe()
    }

    // PlainTcpPort do not support reload with old runtime/notifier
    fn _reload_config_notify_runtime(&self) {}
    fn _reload_escaper_notify_runtime(&self) {}
    fn _reload_user_group_notify_runtime(&self) {}
    fn _reload_auditor_notify_runtime(&self) {}

    fn _reload_with_old_notifier(&self, config: AnyServerConfig) -> anyhow::Result<ArcServer> {
        Err(anyhow!(
            "this {} server doesn't support reload with old notifier",
            config.server_type()
        ))
    }

    fn _reload_with_new_notifier(&self, config: AnyServerConfig) -> anyhow::Result<ArcServer> {
        let server = self.prepare_reload(config)?;
        Ok(Arc::new(server))
    }

    fn _start_runtime(&self, server: &ArcServer) -> anyhow::Result<()> {
        let cur_config = self.config.load();
        let runtime = AuxiliaryTcpPortRuntime::new(server, cur_config.as_ref());
        let listen_config = cur_config.listen.clone();
        let listen_in_worker = cur_config.listen_in_worker;
        drop(cur_config);
        runtime.run_all_instances(&listen_config, listen_in_worker, &self.cfg_sender)
    }

    fn _abort_runtime(&self) {
        let _ = self.reload_sender.send(ServerReloadCommand::QuitRuntime);
        self.cfg_sender.send_replace(None);
    }
}

#[async_trait]
impl Server for PlainTcpPort {
    #[inline]
    fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    fn version(&self) -> usize {
        self.reload_version
    }

    fn escaper(&self) -> String {
        String::new()
    }

    fn user_group(&self) -> String {
        String::new()
    }

    fn auditor(&self) -> String {
        String::new()
    }

    fn get_listen_stats(&self) -> Arc<ListenStats> {
        Arc::clone(&self.listen_stats)
    }

    fn alive_count(&self) -> i32 {
        0
    }

    #[inline]
    fn quit_policy(&self) -> &Arc<ServerQuitPolicy> {
        &self.quit_policy
    }

    async fn run_tcp_task(
        &self,
        _stream: TcpStream,
        _peer_addr: SocketAddr,
        _local_addr: SocketAddr,
        _ctx: ServerRunContext,
    ) {
    }

    async fn run_tls_task(
        &self,
        _stream: TlsStream<TcpStream>,
        _peer_addr: SocketAddr,
        _local_addr: SocketAddr,
        _ctx: ServerRunContext,
    ) {
    }
}
