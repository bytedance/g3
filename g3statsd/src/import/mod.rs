/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;

#[cfg(unix)]
use tokio::net::unix::SocketAddr as UnixSocketAddr;
use tokio::sync::broadcast;

use g3_daemon::listen::ReceiveUdpServer;
#[cfg(unix)]
use g3_daemon::listen::ReceiveUnixDatagramServer;
use g3_daemon::server::{BaseServer, ReloadServer, ServerReloadCommand};
use g3_types::metrics::NodeName;

use crate::config::importer::AnyImporterConfig;

mod registry;
use registry::ImporterRegistry;
pub(crate) use registry::get_names;

mod ops;
pub(crate) use ops::{reload, update_dependency_to_collector};
pub use ops::{spawn_all, stop_all};

mod dummy;
mod statsd;

#[cfg(unix)]
pub(crate) trait Importer:
    ReceiveUdpServer + ReceiveUnixDatagramServer + BaseServer
{
    fn collector(&self) -> &NodeName;
}
#[cfg(not(unix))]
pub(crate) trait Importer: ReceiveUdpServer + BaseServer {
    fn collector(&self) -> &NodeName;
}

trait ImporterInternal: Importer {
    fn _clone_config(&self) -> AnyImporterConfig;

    fn _reload_config_notify_runtime(&self);
    fn _update_collector_in_place(&self);

    fn _reload_with_old_notifier(
        &self,
        config: AnyImporterConfig,
        registry: &mut ImporterRegistry,
    ) -> anyhow::Result<ArcImporterInternal>;
    fn _reload_with_new_notifier(
        &self,
        config: AnyImporterConfig,
        registry: &mut ImporterRegistry,
    ) -> anyhow::Result<ArcImporterInternal>;

    fn _start_runtime(&self, server: ArcImporter) -> anyhow::Result<()>;
    fn _abort_runtime(&self);
}

pub(crate) type ArcImporter = Arc<dyn Importer + Send + Sync>;
type ArcImporterInternal = Arc<dyn ImporterInternal + Send + Sync>;

#[derive(Clone)]
struct WrapArcImporter(ArcImporter);

impl BaseServer for WrapArcImporter {
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

impl ReloadServer for WrapArcImporter {
    fn reload(&self) -> Self {
        WrapArcImporter(registry::get_or_insert_default(self.name()))
    }
}

impl ReceiveUdpServer for WrapArcImporter {
    fn receive_udp_packet(
        &self,
        packet: &[u8],
        client_addr: SocketAddr,
        server_addr: SocketAddr,
        worker_id: Option<usize>,
    ) {
        self.0
            .receive_udp_packet(packet, client_addr, server_addr, worker_id)
    }
}

#[cfg(unix)]
impl ReceiveUnixDatagramServer for WrapArcImporter {
    fn receive_unix_packet(&self, packet: &[u8], peer_addr: UnixSocketAddr) {
        self.0.receive_unix_packet(packet, peer_addr)
    }
}

fn new_reload_notify_channel() -> broadcast::Sender<ServerReloadCommand> {
    broadcast::Sender::new(16)
}
