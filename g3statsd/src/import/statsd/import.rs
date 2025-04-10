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
use arc_swap::ArcSwap;
use log::debug;
use tokio::sync::broadcast;

use g3_daemon::listen::{ReceiveUdpRuntime, ReceiveUdpServer};
use g3_daemon::server::{BaseServer, ServerReloadCommand};
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::metrics::NodeName;

use super::StatsdRecordVisitor;
use crate::collect::ArcCollector;
use crate::config::importer::statsd::StatsdImporterConfig;
use crate::config::importer::{AnyImporterConfig, ImporterConfig};
use crate::import::{
    ArcImporter, ArcImporterInternal, Importer, ImporterInternal, ImporterRegistry, WrapArcImporter,
};

pub(crate) struct StatsdImporter {
    config: StatsdImporterConfig,
    ingress_net_filter: Option<AclNetworkRule>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,

    collector: ArcSwap<ArcCollector>,
    reload_version: usize,
}

impl StatsdImporter {
    fn new(config: StatsdImporterConfig, reload_version: usize) -> Self {
        let reload_sender = crate::import::new_reload_notify_channel();

        let ingress_net_filter = config
            .ingress_net_filter
            .as_ref()
            .map(|builder| builder.build());

        let collector = Arc::new(crate::collect::get_or_insert_default(config.collector()));

        StatsdImporter {
            config,
            ingress_net_filter,
            reload_sender,
            collector: ArcSwap::new(collector),
            reload_version,
        }
    }

    pub(crate) fn prepare_initial(
        config: StatsdImporterConfig,
    ) -> anyhow::Result<ArcImporterInternal> {
        let server = StatsdImporter::new(config, 1);
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyImporterConfig) -> anyhow::Result<StatsdImporter> {
        if let AnyImporterConfig::StatsD(config) = config {
            Ok(StatsdImporter::new(config, self.reload_version + 1))
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.importer_type(),
                config.importer_type()
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

impl ImporterInternal for StatsdImporter {
    fn _clone_config(&self) -> AnyImporterConfig {
        AnyImporterConfig::StatsD(self.config.clone())
    }

    fn _reload_config_notify_runtime(&self) {
        let cmd = ServerReloadCommand::ReloadVersion(self.reload_version);
        let _ = self.reload_sender.send(cmd);
    }

    fn _update_collector_in_place(&self) {
        let collector = crate::collect::get_or_insert_default(self.config.collector());
        self.collector.store(Arc::new(collector));
    }

    fn _reload_with_old_notifier(
        &self,
        config: AnyImporterConfig,
        _registry: &mut ImporterRegistry,
    ) -> anyhow::Result<ArcImporterInternal> {
        let mut server = self.prepare_reload(config)?;
        server.reload_sender = self.reload_sender.clone();
        Ok(Arc::new(server))
    }

    fn _reload_with_new_notifier(
        &self,
        config: AnyImporterConfig,
        _registry: &mut ImporterRegistry,
    ) -> anyhow::Result<ArcImporterInternal> {
        let server = self.prepare_reload(config)?;
        Ok(Arc::new(server))
    }

    fn _start_runtime(&self, importer: ArcImporter) -> anyhow::Result<()> {
        let runtime = ReceiveUdpRuntime::new(
            WrapArcImporter(importer.clone()),
            self.config.listen.clone(),
        );
        runtime.run_all_instances(self.config.listen_in_worker, &self.reload_sender)
    }

    fn _abort_runtime(&self) {
        let _ = self.reload_sender.send(ServerReloadCommand::QuitRuntime);
    }
}

impl BaseServer for StatsdImporter {
    #[inline]
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    #[inline]
    fn server_type(&self) -> &'static str {
        self.config.importer_type()
    }

    #[inline]
    fn version(&self) -> usize {
        self.reload_version
    }
}

impl ReceiveUdpServer for StatsdImporter {
    fn receive_packet(
        &self,
        packet: &[u8],
        client_addr: SocketAddr,
        _server_addr: SocketAddr,
        worker_id: Option<usize>,
    ) {
        if self.drop_early(client_addr) {
            return;
        }

        let iter = StatsdRecordVisitor::new(packet);
        for r in iter {
            match r {
                Ok(r) => self.collector.load().add_metric(r, worker_id),
                Err(e) => {
                    debug!("invalid StatsD record from {}: {e}", client_addr);
                }
            }
        }
    }
}

impl Importer for StatsdImporter {
    fn collector(&self) -> &NodeName {
        self.config.collector()
    }
}
