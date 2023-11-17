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

use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use arc_swap::ArcSwap;
use async_trait::async_trait;
use log::debug;
use quinn::{Connecting, Connection};
use tokio::net::TcpStream;
use tokio::runtime::Handle;
use tokio::sync::{broadcast, watch};
use tokio_openssl::SslStream;
use tokio_rustls::server::TlsStream;

use g3_daemon::listen::ListenStats;
use g3_daemon::server::ClientConnectionInfo;
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::metrics::MetricsName;
use g3_types::net::UdpListenConfig;

use crate::config::server::plain_quic_port::{PlainQuicPortConfig, PlainQuicPortUpdateFlags};
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::serve::{
    ArcServer, AuxQuicServerConfig, AuxiliaryQuicPortRuntime, Server, ServerInternal,
    ServerQuitPolicy, ServerReloadCommand,
};

#[derive(Clone)]
struct PlainQuicPortAuxConfig {
    config: Arc<PlainQuicPortConfig>,
    ingress_net_filter: Option<Arc<AclNetworkRule>>,
    listen_stats: Arc<ListenStats>,
    listen_config: Option<UdpListenConfig>,
    quinn_config: Option<quinn::ServerConfig>,
    accept_timeout: Duration,
    offline_rebind_port: Option<u16>,
}

impl AuxQuicServerConfig for PlainQuicPortAuxConfig {
    fn next_server(&self) -> &MetricsName {
        &self.config.server
    }

    fn run_quic_task(
        &self,
        rt_handle: Handle,
        next_server: ArcServer,
        connecting: Connecting,
        cc_info: ClientConnectionInfo,
    ) {
        let ingress_net_filter = self.ingress_net_filter.clone();
        let listen_stats = self.listen_stats.clone();
        let accept_timeout = self.accept_timeout;
        rt_handle.spawn(async move {
            if let Some(filter) = ingress_net_filter {
                let (_, action) = filter.check(cc_info.sock_peer_ip());
                match action {
                    AclAction::Permit | AclAction::PermitAndLog => {}
                    AclAction::Forbid | AclAction::ForbidAndLog => {
                        listen_stats.add_dropped();
                        return;
                    }
                }
            }

            match tokio::time::timeout(accept_timeout, connecting).await {
                Ok(Ok(connection)) => {
                    listen_stats.add_accepted();
                    next_server.run_quic_task(connection, cc_info).await
                }
                Ok(Err(e)) => {
                    listen_stats.add_failed();
                    // TODO may be attack
                    debug!(
                        "{} - {} quic accept error: {e:?}",
                        cc_info.sock_local_addr(),
                        cc_info.sock_peer_addr()
                    );
                }
                Err(_) => {
                    listen_stats.add_failed();
                    // TODO may be attack
                    debug!(
                        "{} - {} quic accept timeout",
                        cc_info.sock_local_addr(),
                        cc_info.sock_peer_addr()
                    );
                }
            }
        });
    }

    #[inline]
    fn take_udp_listen_config(&mut self) -> Option<UdpListenConfig> {
        self.listen_config.take()
    }

    #[inline]
    fn take_quinn_config(&mut self) -> Option<quinn::ServerConfig> {
        self.quinn_config.take()
    }

    #[inline]
    fn offline_rebind_port(&self) -> Option<u16> {
        self.offline_rebind_port
    }
}

pub(crate) struct PlainQuicPort {
    name: MetricsName,
    config: ArcSwap<PlainQuicPortConfig>,
    quinn_config: quinn::ServerConfig,
    listen_stats: Arc<ListenStats>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,

    cfg_sender: watch::Sender<Option<PlainQuicPortAuxConfig>>,

    quit_policy: Arc<ServerQuitPolicy>,
    reload_version: usize,
}

impl PlainQuicPort {
    fn new(
        config: Arc<PlainQuicPortConfig>,
        listen_stats: Arc<ListenStats>,
        reload_version: usize,
    ) -> anyhow::Result<Self> {
        let reload_sender = crate::serve::new_reload_notify_channel();

        let tls_server = config.tls_server.build()?;

        let ingress_net_filter = config
            .ingress_net_filter
            .as_ref()
            .map(|builder| Arc::new(builder.build()));

        let aux_config = PlainQuicPortAuxConfig {
            config: Arc::clone(&config),
            ingress_net_filter,
            listen_stats: Arc::clone(&listen_stats),
            listen_config: None,
            quinn_config: None,
            accept_timeout: tls_server.accept_timeout,
            offline_rebind_port: config.offline_rebind_port,
        };
        let (cfg_sender, _cfg_receiver) = watch::channel(Some(aux_config));

        Ok(PlainQuicPort {
            name: config.name().clone(),
            config: ArcSwap::new(config),
            quinn_config: quinn::ServerConfig::with_crypto(tls_server.driver),
            listen_stats,
            reload_sender,
            cfg_sender,
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            reload_version,
        })
    }

    pub(crate) fn prepare_initial(config: PlainQuicPortConfig) -> anyhow::Result<ArcServer> {
        let config = Arc::new(config);
        let listen_stats = Arc::new(ListenStats::new(config.name()));

        let server = PlainQuicPort::new(config, listen_stats, 1)?;
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<PlainQuicPort> {
        if let AnyServerConfig::PlainQuicPort(config) = config {
            let config = Arc::new(config);
            let listen_stats = Arc::clone(&self.listen_stats);

            PlainQuicPort::new(config, listen_stats, self.reload_version + 1)
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

impl ServerInternal for PlainQuicPort {
    fn _clone_config(&self) -> AnyServerConfig {
        let cur_config = self.config.load();
        AnyServerConfig::PlainQuicPort(cur_config.as_ref().clone())
    }

    fn _update_config_in_place(&self, flags: u64, config: AnyServerConfig) -> anyhow::Result<()> {
        if let AnyServerConfig::PlainQuicPort(config) = config {
            let config = Arc::new(config);
            let Some(flags) = PlainQuicPortUpdateFlags::from_bits(flags) else {
                return Err(anyhow!("unknown update flags: {flags}"));
            };
            let quinn_config = if flags.contains(PlainQuicPortUpdateFlags::QUINN) {
                let tls_config = config.tls_server.build()?;
                Some(quinn::ServerConfig::with_crypto(tls_config.driver))
            } else {
                None
            };
            let listen_config = if flags.contains(PlainQuicPortUpdateFlags::LISTEN) {
                Some(config.listen.clone())
            } else {
                None
            };
            let ingress_net_filter = config
                .ingress_net_filter
                .as_ref()
                .map(|builder| Arc::new(builder.build()));
            let aux_config = PlainQuicPortAuxConfig {
                config: Arc::clone(&config),
                ingress_net_filter,
                listen_stats: Arc::clone(&self.listen_stats),
                listen_config,
                quinn_config,
                accept_timeout: config.tls_server.accept_timeout(),
                offline_rebind_port: config.offline_rebind_port,
            };
            self.cfg_sender.send_replace(Some(aux_config));
            self.config.store(config);
            Ok(())
        } else {
            Err(anyhow!("invalid config type for PlainQuicPort server"))
        }
    }

    fn _depend_on_server(&self, name: &MetricsName) -> bool {
        let config = self.config.load();
        let config = config.as_ref();
        config.server.eq(name)
    }

    fn _get_reload_notifier(&self) -> broadcast::Receiver<ServerReloadCommand> {
        self.reload_sender.subscribe()
    }

    // PlainTcpPort do not support reload with old runtime/notifier
    fn _reload_config_notify_runtime(&self) {}

    fn _update_next_servers_in_place(&self) {
        // TODO
        todo!()
    }

    fn _update_escaper_in_place(&self) {}
    fn _update_user_group_in_place(&self) {}
    fn _update_audit_handle_in_place(&self) -> anyhow::Result<()> {
        Ok(())
    }

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
        let runtime =
            AuxiliaryQuicPortRuntime::new(server, cur_config.as_ref(), cur_config.listen.clone());
        let listen_in_worker = cur_config.listen_in_worker;
        drop(cur_config);
        runtime.run_all_instances(listen_in_worker, &self.quinn_config, &self.cfg_sender)
    }

    fn _abort_runtime(&self) {
        let _ = self.reload_sender.send(ServerReloadCommand::QuitRuntime);
        self.cfg_sender.send_replace(None);
    }
}

#[async_trait]
impl Server for PlainQuicPort {
    #[inline]
    fn name(&self) -> &MetricsName {
        &self.name
    }

    #[inline]
    fn version(&self) -> usize {
        self.reload_version
    }

    fn escaper(&self) -> &MetricsName {
        Default::default()
    }

    fn user_group(&self) -> &MetricsName {
        Default::default()
    }

    fn auditor(&self) -> &MetricsName {
        Default::default()
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

    async fn run_tcp_task(&self, _stream: TcpStream, _cc_info: ClientConnectionInfo) {}

    async fn run_rustls_task(&self, _stream: TlsStream<TcpStream>, _cc_info: ClientConnectionInfo) {
    }

    async fn run_openssl_task(
        &self,
        _stream: SslStream<TcpStream>,
        _cc_info: ClientConnectionInfo,
    ) {
    }

    async fn run_quic_task(&self, _connection: Connection, _cc_info: ClientConnectionInfo) {}
}
