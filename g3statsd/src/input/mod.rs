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

use crate::config::input::AnyInputConfig;

mod registry;

mod ops;
pub use ops::spawn_all;

mod dummy;
mod statsd;

pub(crate) trait InputInternal {
    fn _clone_config(&self) -> AnyInputConfig;

    fn _reload_config_notify_runtime(&self);

    fn _reload_with_old_notifier(&self, config: AnyInputConfig) -> anyhow::Result<ArcInput>;
    fn _reload_with_new_notifier(&self, config: AnyInputConfig) -> anyhow::Result<ArcInput>;

    fn _start_runtime(&self, server: &ArcInput) -> anyhow::Result<()>;
    fn _abort_runtime(&self);
}

pub(crate) trait Input: InputInternal + ReceiveUdpServer + BaseServer {}

pub(crate) type ArcInput = Arc<dyn Input + Send + Sync>;

#[derive(Clone)]
struct WrapArcInput(ArcInput);

impl BaseServer for WrapArcInput {
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

impl ReloadServer for WrapArcInput {
    fn reload(&self) -> Self {
        WrapArcInput(registry::get_or_insert_default(self.name()))
    }
}

impl ReceiveUdpServer for WrapArcInput {
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
