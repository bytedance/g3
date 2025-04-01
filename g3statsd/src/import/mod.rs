/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use tokio::sync::broadcast;

use g3_daemon::listen::ReceiveUdpServer;
use g3_daemon::server::{BaseServer, ReloadServer, ServerReloadCommand};
use g3_types::metrics::NodeName;

use crate::config::importer::AnyImporterConfig;

mod registry;
pub(crate) use registry::get_names;

mod ops;
pub(crate) use ops::{reload, update_dependency_to_collector};
pub use ops::{spawn_all, stop_all};

mod dummy;
mod statsd;

pub(crate) trait ImporterInternal {
    fn _clone_config(&self) -> AnyImporterConfig;

    fn _reload_config_notify_runtime(&self);
    fn _update_collector_in_place(&self);

    fn _reload_with_old_notifier(&self, config: AnyImporterConfig) -> anyhow::Result<ArcImporter>;
    fn _reload_with_new_notifier(&self, config: AnyImporterConfig) -> anyhow::Result<ArcImporter>;

    fn _start_runtime(&self, server: &ArcImporter) -> anyhow::Result<()>;
    fn _abort_runtime(&self);
}

pub(crate) trait Importer: ImporterInternal + ReceiveUdpServer + BaseServer {
    fn collector(&self) -> &NodeName;
}

pub(crate) type ArcImporter = Arc<dyn Importer + Send + Sync>;

#[derive(Clone)]
struct WrapArcImporter(ArcImporter);

impl BaseServer for WrapArcImporter {
    fn name(&self) -> &NodeName {
        self.0.name()
    }

    fn server_type(&self) -> &'static str {
        self.0.server_type()
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
    fn receive_packet(
        &self,
        packet: &[u8],
        client_addr: SocketAddr,
        server_addr: SocketAddr,
        worker_id: Option<usize>,
    ) {
        self.0
            .receive_packet(packet, client_addr, server_addr, worker_id)
    }
}

fn new_reload_notify_channel() -> broadcast::Sender<ServerReloadCommand> {
    broadcast::Sender::new(16)
}
