/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use async_trait::async_trait;
#[cfg(feature = "quic")]
use quinn::Connection;
use tokio::net::TcpStream;
use tokio::sync::broadcast;

use g3_daemon::listen::{AcceptQuicServer, AcceptTcpServer, ListenStats};
use g3_daemon::server::{
    BaseServer, ClientConnectionInfo, ReloadServer, ServerQuitPolicy, ServerReloadCommand,
};
use g3_types::metrics::NodeName;

use crate::config::server::AnyServerConfig;

mod registry;
use registry::ServerRegistry;
pub(crate) use registry::{get_names, get_or_insert_default};

mod error;
pub(crate) use error::{ServerTaskError, ServerTaskResult};

mod dummy_close;
#[cfg(feature = "quic")]
mod plain_quic_port;
mod plain_tcp_port;

mod keyless_proxy;
mod openssl_proxy;
mod rustls_proxy;

mod ops;
pub(crate) use ops::{
    force_quit_offline_server, force_quit_offline_servers, foreach_server, get_server, reload,
    stop_all, update_dependency_to_backend, wait_all_tasks,
};
pub use ops::{spawn_all, spawn_offline_clean};

mod task;
pub(crate) use task::{ServerTaskNotes, ServerTaskStage};

mod stats;
pub(crate) use stats::{ArcServerStats, ServerStats};

#[async_trait]
pub(crate) trait Server: BaseServer + AcceptTcpServer + AcceptQuicServer {
    fn get_server_stats(&self) -> Option<ArcServerStats> {
        None
    }
    fn get_listen_stats(&self) -> Arc<ListenStats>;

    fn alive_count(&self) -> i32;
    fn quit_policy(&self) -> &Arc<ServerQuitPolicy>;

    fn update_backend(&self, name: &NodeName);
}

trait ServerInternal: Server {
    fn _clone_config(&self) -> AnyServerConfig;
    fn _update_config_in_place(&self, _flags: u64, _config: AnyServerConfig) -> anyhow::Result<()> {
        Ok(())
    }

    fn _depend_on_server(&self, name: &NodeName) -> bool;
    fn _reload_config_notify_runtime(&self);
    fn _update_next_servers_in_place(&self);

    fn _reload_with_old_notifier(
        &self,
        config: AnyServerConfig,
        registry: &mut ServerRegistry,
    ) -> anyhow::Result<ArcServerInternal>;
    fn _reload_with_new_notifier(
        &self,
        config: AnyServerConfig,
        registry: &mut ServerRegistry,
    ) -> anyhow::Result<ArcServerInternal>;

    fn _start_runtime(&self, server: ArcServer) -> anyhow::Result<()>;
    fn _abort_runtime(&self);
}

pub(crate) type ArcServer = Arc<dyn Server + Send + Sync>;
type ArcServerInternal = Arc<dyn ServerInternal + Send + Sync>;

#[derive(Clone)]
struct WrapArcServer(ArcServer);

impl BaseServer for WrapArcServer {
    fn name(&self) -> &NodeName {
        self.0.name()
    }

    fn r#type(&self) -> &'static str {
        self.0.r#type()
    }

    fn version(&self) -> usize {
        self.0.version()
    }
}

impl ReloadServer for WrapArcServer {
    fn reload(&self) -> Self {
        WrapArcServer(get_or_insert_default(self.name()))
    }
}

#[async_trait]
impl AcceptTcpServer for WrapArcServer {
    async fn run_tcp_task(&self, stream: TcpStream, cc_info: ClientConnectionInfo) {
        self.0.run_tcp_task(stream, cc_info).await
    }
}

#[async_trait]
impl AcceptQuicServer for WrapArcServer {
    #[cfg(feature = "quic")]
    async fn run_quic_task(&self, connection: Connection, cc_info: ClientConnectionInfo) {
        self.0.run_quic_task(connection, cc_info).await
    }
}

fn new_reload_notify_channel() -> broadcast::Sender<ServerReloadCommand> {
    broadcast::Sender::new(16)
}
