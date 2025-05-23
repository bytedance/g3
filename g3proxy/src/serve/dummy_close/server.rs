/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
#[cfg(feature = "quic")]
use quinn::Connection;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_rustls::server::TlsStream;

use g3_daemon::listen::{AcceptQuicServer, AcceptTcpServer, ListenStats};
use g3_daemon::server::{BaseServer, ClientConnectionInfo, ServerReloadCommand};
use g3_openssl::SslStream;
use g3_types::metrics::NodeName;

use crate::config::server::dummy_close::DummyCloseServerConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::serve::{
    ArcServer, ArcServerInternal, Server, ServerInternal, ServerQuitPolicy, ServerRegistry,
};

pub(crate) struct DummyCloseServer {
    config: DummyCloseServerConfig,
    listen_stats: Arc<ListenStats>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,

    quit_policy: Arc<ServerQuitPolicy>,
}

impl DummyCloseServer {
    fn new(config: DummyCloseServerConfig, listen_stats: Arc<ListenStats>) -> Self {
        let reload_sender = crate::serve::new_reload_notify_channel();

        DummyCloseServer {
            config,
            listen_stats,
            reload_sender,
            quit_policy: Arc::new(ServerQuitPolicy::default()),
        }
    }

    pub(crate) fn prepare_initial(
        config: DummyCloseServerConfig,
    ) -> anyhow::Result<ArcServerInternal> {
        let listen_stats = Arc::new(ListenStats::new(config.name()));

        let server = DummyCloseServer::new(config, listen_stats);
        Ok(Arc::new(server))
    }

    pub(crate) fn prepare_default(name: &NodeName) -> ArcServerInternal {
        let config = DummyCloseServerConfig::new(name, None);
        let listen_stats = Arc::new(ListenStats::new(name));
        Arc::new(DummyCloseServer::new(config, listen_stats))
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<DummyCloseServer> {
        if let AnyServerConfig::DummyClose(config) = config {
            let listen_stats = Arc::clone(&self.listen_stats);

            let server = DummyCloseServer::new(config, listen_stats);
            Ok(server)
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.r#type(),
                config.r#type()
            ))
        }
    }
}

impl ServerInternal for DummyCloseServer {
    fn _clone_config(&self) -> AnyServerConfig {
        AnyServerConfig::DummyClose(self.config.clone())
    }

    fn _depend_on_server(&self, _name: &NodeName) -> bool {
        false
    }

    // DummyClose server do not support reload with old runtime/notifier
    fn _reload_config_notify_runtime(&self) {}

    fn _update_next_servers_in_place(&self) {}

    fn _update_escaper_in_place(&self) {}

    fn _update_user_group_in_place(&self) {}

    fn _update_audit_handle_in_place(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn _reload_with_old_notifier(
        &self,
        config: AnyServerConfig,
        _registry: &mut ServerRegistry,
    ) -> anyhow::Result<ArcServerInternal> {
        Err(anyhow!(
            "this {} server doesn't support reload with old notifier",
            config.r#type()
        ))
    }

    fn _reload_with_new_notifier(
        &self,
        config: AnyServerConfig,
        _registry: &mut ServerRegistry,
    ) -> anyhow::Result<ArcServerInternal> {
        let server = self.prepare_reload(config)?;
        Ok(Arc::new(server))
    }

    fn _start_runtime(&self, _server: ArcServer) -> anyhow::Result<()> {
        Ok(())
    }

    fn _abort_runtime(&self) {
        let _ = self.reload_sender.send(ServerReloadCommand::QuitRuntime);
    }
}

impl BaseServer for DummyCloseServer {
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    fn r#type(&self) -> &'static str {
        self.config.r#type()
    }

    fn version(&self) -> usize {
        0
    }
}

#[async_trait]
impl AcceptTcpServer for DummyCloseServer {
    async fn run_tcp_task(&self, _stream: TcpStream, _cc_info: ClientConnectionInfo) {}
}

#[async_trait]
impl AcceptQuicServer for DummyCloseServer {
    #[cfg(feature = "quic")]
    async fn run_quic_task(&self, _connection: Connection, _cc_info: ClientConnectionInfo) {}
}

#[async_trait]
impl Server for DummyCloseServer {
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
