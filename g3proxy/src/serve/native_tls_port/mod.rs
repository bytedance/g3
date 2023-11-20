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

use anyhow::{anyhow, Context};
use arc_swap::ArcSwap;
use async_trait::async_trait;
use log::debug;
use openssl::ssl::Ssl;
#[cfg(feature = "quic")]
use quinn::Connection;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_rustls::server::TlsStream;

use g3_daemon::listen::{AcceptQuicServer, AcceptTcpServer, ListenStats, ListenTcpRuntime};
use g3_daemon::server::{BaseServer, ClientConnectionInfo, ServerReloadCommand};
use g3_io_ext::haproxy::{ProxyProtocolV1Reader, ProxyProtocolV2Reader};
use g3_openssl::{SslAcceptor, SslStream};
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::metrics::MetricsName;
use g3_types::net::{OpensslServerConfig, ProxyProtocolVersion};

use crate::config::server::native_tls_port::NativeTlsPortConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::serve::{ArcServer, Server, ServerInternal, ServerQuitPolicy, WrapArcServer};

pub(crate) struct NativeTlsPort {
    config: NativeTlsPortConfig,
    listen_stats: Arc<ListenStats>,
    tls_server_config: OpensslServerConfig,
    ingress_net_filter: Option<AclNetworkRule>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,

    next_server: ArcSwap<ArcServer>,
    quit_policy: Arc<ServerQuitPolicy>,
    reload_version: usize,
}

impl NativeTlsPort {
    fn new(
        config: NativeTlsPortConfig,
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
            .map(|builder| builder.build());

        let next_server = Arc::new(crate::serve::get_or_insert_default(&config.server));

        Ok(NativeTlsPort {
            config,
            listen_stats,
            tls_server_config,
            ingress_net_filter,
            reload_sender,
            next_server: ArcSwap::new(next_server),
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            reload_version,
        })
    }

    pub(crate) fn prepare_initial(config: NativeTlsPortConfig) -> anyhow::Result<ArcServer> {
        let listen_stats = Arc::new(ListenStats::new(config.name()));

        let server = NativeTlsPort::new(config, listen_stats, 1)?;
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<NativeTlsPort> {
        if let AnyServerConfig::NativeTlsPort(config) = config {
            let listen_stats = Arc::clone(&self.listen_stats);

            NativeTlsPort::new(config, listen_stats, self.reload_version + 1)
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

    async fn run_task(&self, mut stream: TcpStream, mut cc_info: ClientConnectionInfo) {
        let Ok(ssl) = Ssl::new(&self.tls_server_config.ssl_context) else {
            self.listen_stats.add_dropped();
            return;
        };

        match self.config.proxy_protocol {
            Some(ProxyProtocolVersion::V1) => {
                let mut parser =
                    ProxyProtocolV1Reader::new(self.config.proxy_protocol_read_timeout);
                match parser.read_proxy_protocol_v1_for_tcp(&mut stream).await {
                    Ok(Some(a)) => cc_info.set_proxy_addr(a),
                    Ok(None) => {}
                    Err(e) => {
                        self.listen_stats.add_by_proxy_protocol_error(e);
                        return;
                    }
                }
            }
            Some(ProxyProtocolVersion::V2) => {
                let mut parser =
                    ProxyProtocolV2Reader::new(self.config.proxy_protocol_read_timeout);
                match parser.read_proxy_protocol_v2_for_tcp(&mut stream).await {
                    Ok(Some(a)) => cc_info.set_proxy_addr(a),
                    Ok(None) => {}
                    Err(e) => {
                        self.listen_stats.add_by_proxy_protocol_error(e);
                        return;
                    }
                }
            }
            None => {}
        }

        let Ok(ssl_acceptor) = SslAcceptor::new(ssl, stream) else {
            self.listen_stats.add_dropped();
            return;
        };
        match tokio::time::timeout(self.tls_server_config.accept_timeout, ssl_acceptor.accept())
            .await
        {
            Ok(Ok(ssl_stream)) => {
                let next_server = self.next_server.load().as_ref().clone();
                next_server.run_openssl_task(ssl_stream, cc_info).await
            }
            Ok(Err(e)) => {
                self.listen_stats.add_failed();
                debug!(
                    "{} - {} tls error: {e:?}",
                    cc_info.sock_local_addr(),
                    cc_info.sock_peer_addr()
                );
                // TODO record tls failure and add some sec policy
            }
            Err(_) => {
                self.listen_stats.add_timeout();
                debug!(
                    "{} - {} tls timeout",
                    cc_info.sock_local_addr(),
                    cc_info.sock_peer_addr()
                );
                // TODO record tls failure and add some sec policy
            }
        }
    }
}

impl ServerInternal for NativeTlsPort {
    fn _clone_config(&self) -> AnyServerConfig {
        AnyServerConfig::NativeTlsPort(self.config.clone())
    }

    fn _update_config_in_place(&self, _flags: u64, _config: AnyServerConfig) -> anyhow::Result<()> {
        Ok(())
    }

    fn _depend_on_server(&self, name: &MetricsName) -> bool {
        self.config.server.eq(name)
    }

    fn _reload_config_notify_runtime(&self) {
        let cmd = ServerReloadCommand::ReloadVersion(self.reload_version);
        let _ = self.reload_sender.send(cmd);
    }

    fn _update_next_servers_in_place(&self) {
        let next_server = crate::serve::get_or_insert_default(&self.config.server);
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
        let runtime =
            ListenTcpRuntime::new(WrapArcServer(server.clone()), server.get_listen_stats());
        runtime.run_all_instances(
            &self.config.listen,
            self.config.listen_in_worker,
            &self.reload_sender,
        )
    }

    fn _abort_runtime(&self) {
        let _ = self.reload_sender.send(ServerReloadCommand::QuitRuntime);
    }
}

impl BaseServer for NativeTlsPort {
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
impl AcceptTcpServer for NativeTlsPort {
    async fn run_tcp_task(&self, stream: TcpStream, cc_info: ClientConnectionInfo) {
        let client_addr = cc_info.client_addr();
        if self.drop_early(client_addr) {
            return;
        }

        self.run_task(stream, cc_info).await
    }
}

#[async_trait]
impl AcceptQuicServer for NativeTlsPort {
    #[cfg(feature = "quic")]
    async fn run_quic_task(&self, _connection: Connection, _cc_info: ClientConnectionInfo) {}
}

#[async_trait]
impl Server for NativeTlsPort {
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

    async fn run_rustls_task(&self, _stream: TlsStream<TcpStream>, _cc_info: ClientConnectionInfo) {
    }

    async fn run_openssl_task(
        &self,
        _stream: SslStream<TcpStream>,
        _cc_info: ClientConnectionInfo,
    ) {
    }
}
