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

use crate::config::importer::dummy::DummyImporterConfig;
use crate::config::importer::{AnyImporterConfig, ImporterConfig};
use crate::import::{ArcImporter, Importer, ImporterInternal};

pub(crate) struct DummyImporter {
    config: DummyImporterConfig,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
}

impl DummyImporter {
    fn new(config: DummyImporterConfig) -> Self {
        let reload_sender = crate::import::new_reload_notify_channel();

        DummyImporter {
            config,
            reload_sender,
        }
    }

    pub(crate) fn prepare_initial(config: DummyImporterConfig) -> anyhow::Result<ArcImporter> {
        let server = DummyImporter::new(config);
        Ok(Arc::new(server))
    }

    pub(crate) fn prepare_default(name: &NodeName) -> ArcImporter {
        let config = DummyImporterConfig::with_name(name, None);
        Arc::new(DummyImporter::new(config))
    }

    fn prepare_reload(&self, config: AnyImporterConfig) -> anyhow::Result<DummyImporter> {
        if let AnyImporterConfig::Dummy(config) = config {
            Ok(DummyImporter::new(config))
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.importer_type(),
                config.importer_type()
            ))
        }
    }
}

impl ImporterInternal for DummyImporter {
    fn _clone_config(&self) -> AnyImporterConfig {
        AnyImporterConfig::Dummy(self.config.clone())
    }

    fn _reload_config_notify_runtime(&self) {
        let cmd = ServerReloadCommand::ReloadVersion(0);
        let _ = self.reload_sender.send(cmd);
    }

    fn _reload_with_old_notifier(&self, config: AnyImporterConfig) -> anyhow::Result<ArcImporter> {
        Err(anyhow!(
            "this {} importer doesn't support reload with old notifier",
            config.importer_type()
        ))
    }

    fn _reload_with_new_notifier(&self, config: AnyImporterConfig) -> anyhow::Result<ArcImporter> {
        let importer = self.prepare_reload(config)?;
        Ok(Arc::new(importer))
    }

    fn _start_runtime(&self, _importer: &ArcImporter) -> anyhow::Result<()> {
        Ok(())
    }

    fn _abort_runtime(&self) {}
}

impl BaseServer for DummyImporter {
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
        0
    }
}

impl ReceiveUdpServer for DummyImporter {
    fn receive_packet(
        &self,
        _packet: &[u8],
        _client_addr: SocketAddr,
        _server_addr: SocketAddr,
        _worker_id: Option<usize>,
    ) {
    }
}

impl Importer for DummyImporter {}
