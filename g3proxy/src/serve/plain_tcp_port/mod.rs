/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::anyhow;
use arc_swap::ArcSwap;
use async_trait::async_trait;
#[cfg(feature = "quic")]
use quinn::Connection;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_rustls::server::TlsStream;

use g3_daemon::listen::{AcceptQuicServer, AcceptTcpServer, ListenStats, ListenTcpRuntime};
use g3_daemon::server::{BaseServer, ClientConnectionInfo, ServerReloadCommand};
use g3_io_ext::haproxy::{ProxyProtocolV1Reader, ProxyProtocolV2Reader};
use g3_openssl::SslStream;
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::metrics::NodeName;
use g3_types::net::ProxyProtocolVersion;

use crate::config::server::plain_tcp_port::PlainTcpPortConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::serve::{
    ArcServer, ArcServerInternal, Server, ServerInternal, ServerQuitPolicy, ServerRegistry,
    WrapArcServer,
};

pub(crate) struct PlainTcpPort {
    config: PlainTcpPortConfig,
    listen_stats: Arc<ListenStats>,
    ingress_net_filter: Option<AclNetworkRule>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,

    next_server: ArcSwap<ArcServer>,
    quit_policy: Arc<ServerQuitPolicy>,
    reload_version: usize,
}

impl PlainTcpPort {
    fn new<F>(
        config: PlainTcpPortConfig,
        listen_stats: Arc<ListenStats>,
        reload_version: usize,
        mut fetch_server: F,
    ) -> anyhow::Result<Self>
    where
        F: FnMut(&NodeName) -> ArcServer,
    {
        let reload_sender = crate::serve::new_reload_notify_channel();

        let ingress_net_filter = config
            .ingress_net_filter
            .as_ref()
            .map(|builder| builder.build());

        let next_server = Arc::new(fetch_server(&config.server));

        Ok(PlainTcpPort {
            config,
            listen_stats,
            ingress_net_filter,
            reload_sender,
            next_server: ArcSwap::new(next_server),
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            reload_version,
        })
    }

    pub(crate) fn prepare_initial(config: PlainTcpPortConfig) -> anyhow::Result<ArcServerInternal> {
        let listen_stats = Arc::new(ListenStats::new(config.name()));

        let server =
            PlainTcpPort::new(config, listen_stats, 1, crate::serve::get_or_insert_default)?;
        Ok(Arc::new(server))
    }

    fn prepare_reload(
        &self,
        config: AnyServerConfig,
        registry: &mut ServerRegistry,
    ) -> anyhow::Result<PlainTcpPort> {
        if let AnyServerConfig::PlainTcpPort(config) = config {
            let listen_stats = Arc::clone(&self.listen_stats);

            PlainTcpPort::new(config, listen_stats, self.reload_version + 1, |name| {
                registry.get_or_insert_default(name)
            })
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

    async fn run_task(&self, mut stream: TcpStream, mut cc_info: ClientConnectionInfo) {
        let next_server = self.next_server.load().as_ref().clone();

        match self.config.proxy_protocol {
            Some(ProxyProtocolVersion::V1) => {
                let mut parser =
                    ProxyProtocolV1Reader::new(self.config.proxy_protocol_read_timeout);
                match parser.read_proxy_protocol_v1_for_tcp(&mut stream).await {
                    Ok(Some(a)) => {
                        cc_info.set_proxy_addr(a);
                        next_server.run_tcp_task(stream, cc_info).await
                    }
                    Ok(None) => next_server.run_tcp_task(stream, cc_info).await,
                    Err(e) => self.listen_stats.add_by_proxy_protocol_error(e),
                }
            }
            Some(ProxyProtocolVersion::V2) => {
                let mut parser =
                    ProxyProtocolV2Reader::new(self.config.proxy_protocol_read_timeout);
                match parser.read_proxy_protocol_v2_for_tcp(&mut stream).await {
                    Ok(Some(a)) => {
                        cc_info.set_proxy_addr(a);
                        next_server.run_tcp_task(stream, cc_info).await
                    }
                    Ok(None) => next_server.run_tcp_task(stream, cc_info).await,
                    Err(e) => self.listen_stats.add_by_proxy_protocol_error(e),
                }
            }
            None => next_server.run_tcp_task(stream, cc_info).await,
        }
    }
}

impl ServerInternal for PlainTcpPort {
    fn _clone_config(&self) -> AnyServerConfig {
        AnyServerConfig::PlainTcpPort(self.config.clone())
    }

    fn _depend_on_server(&self, name: &NodeName) -> bool {
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

    fn _reload_with_old_notifier(
        &self,
        config: AnyServerConfig,
        registry: &mut ServerRegistry,
    ) -> anyhow::Result<ArcServerInternal> {
        let mut server = self.prepare_reload(config, registry)?;
        server.reload_sender = self.reload_sender.clone();
        Ok(Arc::new(server))
    }

    fn _reload_with_new_notifier(
        &self,
        config: AnyServerConfig,
        registry: &mut ServerRegistry,
    ) -> anyhow::Result<ArcServerInternal> {
        let server = self.prepare_reload(config, registry)?;
        Ok(Arc::new(server))
    }

    fn _start_runtime(&self, server: ArcServer) -> anyhow::Result<()> {
        let listen_stats = server.get_listen_stats();
        let runtime = ListenTcpRuntime::new(WrapArcServer(server), listen_stats);
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

impl BaseServer for PlainTcpPort {
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
impl AcceptTcpServer for PlainTcpPort {
    async fn run_tcp_task(&self, stream: TcpStream, cc_info: ClientConnectionInfo) {
        let client_addr = cc_info.client_addr();
        if self.drop_early(client_addr) {
            return;
        }

        self.run_task(stream, cc_info).await
    }
}

#[async_trait]
impl AcceptQuicServer for PlainTcpPort {
    #[cfg(feature = "quic")]
    async fn run_quic_task(&self, _connection: Connection, _cc_info: ClientConnectionInfo) {}
}

#[async_trait]
impl Server for PlainTcpPort {
    fn escaper(&self) -> &NodeName {
        Default::default()
    }

    fn user_group(&self) -> &NodeName {
        Default::default()
    }

    fn auditor(&self) -> &NodeName {
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
