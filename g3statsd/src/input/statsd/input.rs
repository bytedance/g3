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
use log::debug;
use tokio::sync::broadcast;

use g3_daemon::listen::{ReceiveUdpRuntime, ReceiveUdpServer};
use g3_daemon::server::{BaseServer, ServerReloadCommand};
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::metrics::NodeName;

use crate::config::input::statsd::StatsdInputConfig;
use crate::config::input::{AnyInputConfig, InputConfig};
use crate::input::{ArcInput, Input, InputInternal, WrapArcInput};

pub(crate) struct StatsdInput {
    config: StatsdInputConfig,
    ingress_net_filter: Option<AclNetworkRule>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,

    reload_version: usize,
}

impl StatsdInput {
    fn new(config: StatsdInputConfig, reload_version: usize) -> Self {
        let reload_sender = crate::input::new_reload_notify_channel();

        let ingress_net_filter = config
            .ingress_net_filter
            .as_ref()
            .map(|builder| builder.build());

        StatsdInput {
            config,
            ingress_net_filter,
            reload_sender,
            reload_version,
        }
    }

    pub(crate) fn prepare_initial(config: StatsdInputConfig) -> anyhow::Result<ArcInput> {
        let server = StatsdInput::new(config, 1);
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyInputConfig) -> anyhow::Result<StatsdInput> {
        if let AnyInputConfig::StatsD(config) = config {
            Ok(StatsdInput::new(config, self.reload_version + 1))
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.input_type(),
                config.input_type()
            ))
        }
    }

    fn drop_early(&self, client_addr: SocketAddr) -> bool {
        if let Some(ingress_net_filter) = &self.ingress_net_filter {
            let (_, action) = ingress_net_filter.check(client_addr.ip());
            match action {
                AclAction::Permit | AclAction::PermitAndLog => {}
                AclAction::Forbid | AclAction::ForbidAndLog => {
                    return true;
                }
            }
        }

        // TODO add cps limit

        false
    }
}

impl InputInternal for StatsdInput {
    fn _clone_config(&self) -> AnyInputConfig {
        AnyInputConfig::StatsD(self.config.clone())
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

    fn _start_runtime(&self, input: &ArcInput) -> anyhow::Result<()> {
        let runtime =
            ReceiveUdpRuntime::new(WrapArcInput(input.clone()), self.config.listen.clone());
        runtime.run_all_instances(self.config.listen_in_worker, &self.reload_sender)
    }

    fn _abort_runtime(&self) {
        let _ = self.reload_sender.send(ServerReloadCommand::QuitRuntime);
    }
}

impl BaseServer for StatsdInput {
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

impl ReceiveUdpServer for StatsdInput {
    fn receive_packet(
        &self,
        packet: &[u8],
        client_addr: SocketAddr,
        _server_addr: SocketAddr,
        _worker_id: Option<usize>,
    ) {
        if self.drop_early(client_addr) {
            return;
        }

        debug!("received {} bytes from {}", packet.len(), client_addr);
    }
}

impl Input for StatsdInput {}
