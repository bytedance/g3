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
use quinn::Connection;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, watch};
use tokio_rustls::server::TlsStream;

use g3_daemon::listen::{ListenQuicConf, ListenStats};
use g3_daemon::server::{ClientConnectionInfo, ServerReloadCommand};
use g3_openssl::SslStream;
use g3_types::acl::AclNetworkRule;
use g3_types::metrics::MetricsName;
use g3_types::net::UdpListenConfig;

use crate::config::server::plain_quic_port::{PlainQuicPortConfig, PlainQuicPortUpdateFlags};
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::serve::{ArcServer, ListenQuicRuntime, Server, ServerInternal, ServerQuitPolicy};

#[derive(Clone)]
struct PlainQuicPortAuxConfig {
    ingress_net_filter: Option<Arc<AclNetworkRule>>,
    listen_config: Option<UdpListenConfig>,
    quinn_config: Option<quinn::ServerConfig>,
    accept_timeout: Duration,
    offline_rebind_port: Option<u16>,
}

impl ListenQuicConf for PlainQuicPortAuxConfig {
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

    #[inline]
    fn ingress_network_acl(&self) -> Option<&AclNetworkRule> {
        self.ingress_net_filter.as_ref().map(|v| v.as_ref())
    }

    #[inline]
    fn accept_timeout(&self) -> Duration {
        self.accept_timeout
    }
}

pub(crate) struct PlainQuicPort {
    name: MetricsName,
    config: ArcSwap<PlainQuicPortConfig>,
    quinn_config: quinn::ServerConfig,
    listen_stats: Arc<ListenStats>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,

    cfg_sender: watch::Sender<PlainQuicPortAuxConfig>,

    next_server: ArcSwap<ArcServer>,
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

        let next_server = Arc::new(crate::serve::get_or_insert_default(&config.server));

        let aux_config = PlainQuicPortAuxConfig {
            ingress_net_filter,
            listen_config: None,
            quinn_config: None,
            accept_timeout: tls_server.accept_timeout,
            offline_rebind_port: config.offline_rebind_port,
        };
        let (cfg_sender, _cfg_receiver) = watch::channel(aux_config);

        Ok(PlainQuicPort {
            name: config.name().clone(),
            config: ArcSwap::new(config),
            quinn_config: quinn::ServerConfig::with_crypto(tls_server.driver),
            listen_stats,
            reload_sender,
            cfg_sender,
            next_server: ArcSwap::new(next_server),
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            reload_version,
        })
    }

    pub(crate) fn prepare_initial(config: PlainQuicPortConfig) -> anyhow::Result<ArcServer> {
        let listen_stats = Arc::new(ListenStats::new(config.name()));

        let server = PlainQuicPort::new(Arc::new(config), listen_stats, 1)?;
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<PlainQuicPort> {
        if let AnyServerConfig::PlainQuicPort(config) = config {
            let listen_stats = Arc::clone(&self.listen_stats);

            PlainQuicPort::new(Arc::new(config), listen_stats, self.reload_version + 1)
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
        let config = self.config.load();
        AnyServerConfig::PlainQuicPort(config.as_ref().clone())
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
                ingress_net_filter,
                listen_config,
                quinn_config,
                accept_timeout: config.tls_server.accept_timeout(),
                offline_rebind_port: config.offline_rebind_port,
            };
            self.cfg_sender.send_replace(aux_config);
            self.config.store(config);

            if flags.contains(PlainQuicPortUpdateFlags::NEXT_SERVER) {
                self._update_next_servers_in_place();
            }
            Ok(())
        } else {
            Err(anyhow!("invalid config type for PlainQuicPort server"))
        }
    }

    fn _depend_on_server(&self, name: &MetricsName) -> bool {
        let config = self.config.load();
        config.server.eq(name)
    }

    fn _reload_config_notify_runtime(&self) {
        let cmd = ServerReloadCommand::ReloadVersion(self.reload_version);
        let _ = self.reload_sender.send(cmd);
    }

    fn _update_next_servers_in_place(&self) {
        let next_server = crate::serve::get_or_insert_default(&self.config.load().server);
        self.next_server.store(Arc::new(next_server));
    }

    fn _update_escaper_in_place(&self) {}
    fn _update_user_group_in_place(&self) {}
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
        let config = self.config.load();
        let runtime = ListenQuicRuntime::new(server, config.as_ref(), config.listen.clone());
        runtime.run_all_instances(
            config.listen_in_worker,
            &self.quinn_config,
            &self.reload_sender,
            &self.cfg_sender,
        )
    }

    fn _abort_runtime(&self) {
        let _ = self.reload_sender.send(ServerReloadCommand::QuitRuntime);
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

    async fn run_quic_task(&self, connection: Connection, cc_info: ClientConnectionInfo) {
        let next_server = self.next_server.load().as_ref().clone();
        next_server.run_quic_task(connection, cc_info).await;
    }
}
