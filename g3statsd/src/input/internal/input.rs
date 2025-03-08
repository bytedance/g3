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

use crate::config::input::internal::InternalInputConfig;
use crate::config::input::{AnyInputConfig, InputConfig};
use crate::input::{ArcInput, Input, InputInternal};

pub(crate) struct InternalInput {
    config: InternalInputConfig,
    reload_sender: broadcast::Sender<ServerReloadCommand>,

    reload_version: usize,
}

impl InternalInput {
    fn new(config: InternalInputConfig, reload_version: usize) -> Self {
        let reload_sender = crate::input::new_reload_notify_channel();

        InternalInput {
            config,
            reload_sender,
            reload_version,
        }
    }

    pub(crate) fn prepare_initial(config: InternalInputConfig) -> anyhow::Result<ArcInput> {
        let server = InternalInput::new(config, 1);
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyInputConfig) -> anyhow::Result<InternalInput> {
        if let AnyInputConfig::Internal(config) = config {
            Ok(InternalInput::new(config, self.reload_version + 1))
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.input_type(),
                config.input_type()
            ))
        }
    }
}

impl InputInternal for InternalInput {
    fn _clone_config(&self) -> AnyInputConfig {
        AnyInputConfig::Internal(self.config.clone())
    }

    fn _update_config_in_place(&self, flags: u64, config: AnyInputConfig) -> anyhow::Result<()> {
        todo!()
    }

    fn _reload_config_notify_runtime(&self) {
        let cmd = ServerReloadCommand::ReloadVersion(self.reload_version);
        let _ = self.reload_sender.send(cmd);
    }

    fn _reload_with_old_notifier(&self, config: AnyInputConfig) -> anyhow::Result<ArcInput> {
        let mut server = self.prepare_reload(config)?;
        server.reload_sender = self.reload_sender.clone();
        Ok(Arc::new(server))
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

impl BaseServer for InternalInput {
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
        self.reload_version
    }
}

impl ReceiveUdpServer for InternalInput {
    fn receive_packet(
        &self,
        _packet: &[u8],
        _client_addr: SocketAddr,
        _server_addr: SocketAddr,
        _worker_id: Option<usize>,
    ) {
    }
}

impl Input for InternalInput {}
