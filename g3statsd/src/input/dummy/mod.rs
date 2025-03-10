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

use anyhow::anyhow;
use tokio::sync::broadcast;

use g3_daemon::listen::ReceiveUdpServer;
use g3_daemon::server::{BaseServer, ServerReloadCommand};
use g3_types::metrics::NodeName;

use crate::config::input::dummy::DummyInputConfig;
use crate::config::input::{AnyInputConfig, InputConfig};
use crate::input::{ArcInput, Input, InputInternal};

pub(crate) struct DummyInput {
    config: DummyInputConfig,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
}

impl DummyInput {
    fn new(config: DummyInputConfig) -> Self {
        let reload_sender = crate::input::new_reload_notify_channel();

        DummyInput {
            config,
            reload_sender,
        }
    }

    pub(crate) fn prepare_initial(config: DummyInputConfig) -> anyhow::Result<ArcInput> {
        let server = DummyInput::new(config);
        Ok(Arc::new(server))
    }

    pub(crate) fn prepare_default(name: &NodeName) -> ArcInput {
        let config = DummyInputConfig::with_name(name, None);
        Arc::new(DummyInput::new(config))
    }

    fn prepare_reload(&self, config: AnyInputConfig) -> anyhow::Result<DummyInput> {
        if let AnyInputConfig::Dummy(config) = config {
            Ok(DummyInput::new(config))
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.input_type(),
                config.input_type()
            ))
        }
    }
}

impl InputInternal for DummyInput {
    fn _clone_config(&self) -> AnyInputConfig {
        AnyInputConfig::Dummy(self.config.clone())
    }

    fn _reload_config_notify_runtime(&self) {
        let cmd = ServerReloadCommand::ReloadVersion(0);
        let _ = self.reload_sender.send(cmd);
    }

    fn _reload_with_old_notifier(&self, config: AnyInputConfig) -> anyhow::Result<ArcInput> {
        Err(anyhow!(
            "this {} input doesn't support reload with old notifier",
            config.input_type()
        ))
    }

    fn _reload_with_new_notifier(&self, config: AnyInputConfig) -> anyhow::Result<ArcInput> {
        let server = self.prepare_reload(config)?;
        Ok(Arc::new(server))
    }

    fn _start_runtime(&self, _input: &ArcInput) -> anyhow::Result<()> {
        Ok(())
    }

    fn _abort_runtime(&self) {}
}

impl BaseServer for DummyInput {
    #[inline]
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    #[inline]
    fn server_type(&self) -> &'static str {
        self.config.input_type()
    }

    #[inline]
    fn version(&self) -> usize {
        0
    }
}

impl ReceiveUdpServer for DummyInput {
    fn receive_packet(
        &self,
        _packet: &[u8],
        _client_addr: SocketAddr,
        _server_addr: SocketAddr,
        _worker_id: Option<usize>,
    ) {
    }
}

impl Input for DummyInput {}
