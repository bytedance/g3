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

use std::pin::Pin;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use arc_swap::ArcSwap;
use async_trait::async_trait;
use log::debug;
use openssl::ssl::Ssl;
use tokio::net::TcpStream;
use tokio::runtime::Handle;
use tokio::sync::{broadcast, watch};
use tokio_openssl::SslStream;
use tokio_rustls::server::TlsStream;

use g3_daemon::listen::ListenStats;
use g3_daemon::server::ClientConnectionInfo;
use g3_io_ext::haproxy::{ProxyProtocolV1Reader, ProxyProtocolV2Reader};
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::metrics::MetricsName;
use g3_types::net::{OpensslServerConfig, ProxyProtocolVersion};

use crate::config::server::native_tls_port::NativeTlsPortConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::serve::{
    ArcServer, AuxiliaryServerConfig, AuxiliaryTcpPortRuntime, Server, ServerInternal,
    ServerQuitPolicy, ServerReloadCommand, ServerRunContext,
};

#[derive(Clone)]
struct NativeTlsPortAuxConfig {
    config: Arc<NativeTlsPortConfig>,
    tls_server_config: OpensslServerConfig,
    ingress_net_filter: Option<Arc<AclNetworkRule>>,
    listen_stats: Arc<ListenStats>,
}

impl AuxiliaryServerConfig for NativeTlsPortAuxConfig {
    fn next_server(&self) -> &MetricsName {
        &self.config.server
    }

    fn run_tcp_task(
        &self,
        rt_handle: Handle,
        next_server: ArcServer,
        stream: TcpStream,
        cc_info: ClientConnectionInfo,
        ctx: ServerRunContext,
    ) {
        let Ok(ssl) = Ssl::new(&self.tls_server_config.ssl_context) else {
            self.listen_stats.add_dropped();
            return;
        };

        let tls_accept_timeout = self.tls_server_config.accept_timeout;
        let ingress_net_filter = self.ingress_net_filter.clone();
        let listen_stats = Arc::clone(&self.listen_stats);
        let proxy_protocol = self.config.proxy_protocol;
        let proxy_protocol_read_timeout = self.config.proxy_protocol_read_timeout;

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

            let mut stream = stream;
            let mut cc_info = cc_info;
            match proxy_protocol {
                Some(ProxyProtocolVersion::V1) => {
                    let mut parser = ProxyProtocolV1Reader::new(proxy_protocol_read_timeout);
                    match parser.read_proxy_protocol_v1_for_tcp(&mut stream).await {
                        Ok(Some(a)) => cc_info.set_proxy_addr(a),
                        Ok(None) => {}
                        Err(e) => {
                            listen_stats.add_by_proxy_protocol_error(e);
                            return;
                        }
                    }
                }
                Some(ProxyProtocolVersion::V2) => {
                    let mut parser = ProxyProtocolV2Reader::new(proxy_protocol_read_timeout);
                    match parser.read_proxy_protocol_v2_for_tcp(&mut stream).await {
                        Ok(Some(a)) => cc_info.set_proxy_addr(a),
                        Ok(None) => {}
                        Err(e) => {
                            listen_stats.add_by_proxy_protocol_error(e);
                            return;
                        }
                    }
                }
                None => {}
            }

            let Ok(mut ssl_stream) = SslStream::new(ssl, stream) else {
                listen_stats.add_dropped();
                return;
            };
            match tokio::time::timeout(tls_accept_timeout, Pin::new(&mut ssl_stream).accept()).await
            {
                Ok(Ok(_)) => next_server.run_openssl_task(ssl_stream, cc_info, ctx).await,
                Ok(Err(e)) => {
                    listen_stats.add_failed();
                    debug!(
                        "{} - {} tls error: {e:?}",
                        cc_info.sock_local_addr(),
                        cc_info.sock_peer_addr()
                    );
                    // TODO record tls failure and add some sec policy
                }
                Err(_) => {
                    listen_stats.add_timeout();
                    debug!(
                        "{} - {} tls timeout",
                        cc_info.sock_local_addr(),
                        cc_info.sock_peer_addr()
                    );
                    // TODO record tls failure and add some sec policy
                }
            }
        });
    }
}

pub(crate) struct NativeTlsPort {
    name: MetricsName,
    config: ArcSwap<NativeTlsPortConfig>,
    listen_stats: Arc<ListenStats>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,

    cfg_sender: watch::Sender<Option<NativeTlsPortAuxConfig>>,

    quit_policy: Arc<ServerQuitPolicy>,
    reload_version: usize,
}

impl NativeTlsPort {
    fn new(
        config: Arc<NativeTlsPortConfig>,
        listen_stats: Arc<ListenStats>,
        reload_version: usize,
    ) -> anyhow::Result<Self> {
        let reload_sender = crate::serve::new_reload_notify_channel();

        let tls_server_config = if let Some(builder) = &config.server_tls_config {
            builder
                .build()
                .context("failed to build tls server config")?
        } else {
            return Err(anyhow!("no tls server config set"));
        };

        let ingress_net_filter = config
            .ingress_net_filter
            .as_ref()
            .map(|builder| Arc::new(builder.build()));

        let aux_config = NativeTlsPortAuxConfig {
            config: Arc::clone(&config),
            tls_server_config,
            ingress_net_filter,
            listen_stats: Arc::clone(&listen_stats),
        };
        let (cfg_sender, _cfg_receiver) = watch::channel(Some(aux_config));

        Ok(NativeTlsPort {
            name: config.name().clone(),
            config: ArcSwap::new(config),
            listen_stats,
            reload_sender,
            cfg_sender,
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            reload_version,
        })
    }

    pub(crate) fn prepare_initial(config: AnyServerConfig) -> anyhow::Result<ArcServer> {
        if let AnyServerConfig::NativeTlsPort(config) = config {
            let config = Arc::new(config);
            let listen_stats = Arc::new(ListenStats::new(config.name()));

            let server = NativeTlsPort::new(config, listen_stats, 1)?;
            Ok(Arc::new(server))
        } else {
            Err(anyhow!("invalid config type for PlainTcpPort server"))
        }
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<NativeTlsPort> {
        if let AnyServerConfig::NativeTlsPort(config) = config {
            let config = Arc::new(config);
            let listen_stats = Arc::clone(&self.listen_stats);

            NativeTlsPort::new(config, listen_stats, self.reload_version + 1)
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

impl ServerInternal for NativeTlsPort {
    fn _clone_config(&self) -> AnyServerConfig {
        let cur_config = self.config.load();
        AnyServerConfig::NativeTlsPort(cur_config.as_ref().clone())
    }

    fn _update_config_in_place(&self, _flags: u64, config: AnyServerConfig) -> anyhow::Result<()> {
        if let AnyServerConfig::NativeTlsPort(config) = config {
            let config = Arc::new(config);

            let tls_server_config = if let Some(builder) = &config.server_tls_config {
                builder
                    .build()
                    .context("failed to build tls server config")?
            } else {
                return Err(anyhow!("no tls server config set"));
            };

            let ingress_net_filter = config
                .ingress_net_filter
                .as_ref()
                .map(|builder| Arc::new(builder.build()));

            let aux_config = NativeTlsPortAuxConfig {
                config: Arc::clone(&config),
                tls_server_config,
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

    // PlainTlsPort do not support reload with old runtime/notifier
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
impl Server for NativeTlsPort {
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

    async fn run_tcp_task(
        &self,
        _stream: TcpStream,
        _cc_info: ClientConnectionInfo,
        _ctx: ServerRunContext,
    ) {
    }

    async fn run_rustls_task(
        &self,
        _stream: TlsStream<TcpStream>,
        _cc_info: ClientConnectionInfo,
        _ctx: ServerRunContext,
    ) {
    }

    async fn run_openssl_task(
        &self,
        _stream: SslStream<TcpStream>,
        _cc_info: ClientConnectionInfo,
        _ctx: ServerRunContext,
    ) {
    }
}
