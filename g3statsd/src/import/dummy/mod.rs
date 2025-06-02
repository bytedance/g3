/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::anyhow;
#[cfg(unix)]
use tokio::net::unix::SocketAddr as UnixSocketAddr;
use tokio::sync::broadcast;

use g3_daemon::listen::ReceiveUdpServer;
#[cfg(unix)]
use g3_daemon::listen::ReceiveUnixDatagramServer;
use g3_daemon::server::{BaseServer, ServerReloadCommand};
use g3_types::metrics::NodeName;

use super::{ArcImporter, ArcImporterInternal, Importer, ImporterInternal, ImporterRegistry};
use crate::config::importer::dummy::DummyImporterConfig;
use crate::config::importer::{AnyImporterConfig, ImporterConfig};

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

    pub(crate) fn prepare_initial(
        config: DummyImporterConfig,
    ) -> anyhow::Result<ArcImporterInternal> {
        let server = DummyImporter::new(config);
        Ok(Arc::new(server))
    }

    pub(crate) fn prepare_default(name: &NodeName) -> ArcImporterInternal {
        let config = DummyImporterConfig::with_name(name, None);
        Arc::new(DummyImporter::new(config))
    }

    fn prepare_reload(&self, config: AnyImporterConfig) -> anyhow::Result<ArcImporterInternal> {
        if let AnyImporterConfig::Dummy(config) = config {
            Ok(Arc::new(DummyImporter::new(config)))
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

    fn _update_collector_in_place(&self) {}

    fn _reload_with_old_notifier(
        &self,
        config: AnyImporterConfig,
        _registry: &mut ImporterRegistry,
    ) -> anyhow::Result<ArcImporterInternal> {
        Err(anyhow!(
            "this {} importer doesn't support reload with old notifier",
            config.importer_type()
        ))
    }

    fn _reload_with_new_notifier(
        &self,
        config: AnyImporterConfig,
        _registry: &mut ImporterRegistry,
    ) -> anyhow::Result<ArcImporterInternal> {
        self.prepare_reload(config)
    }

    fn _start_runtime(&self, _importer: ArcImporter) -> anyhow::Result<()> {
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
    fn r#type(&self) -> &'static str {
        self.config.importer_type()
    }

    #[inline]
    fn version(&self) -> usize {
        0
    }
}

impl ReceiveUdpServer for DummyImporter {
    fn receive_udp_packet(
        &self,
        _packet: &[u8],
        _client_addr: SocketAddr,
        _server_addr: SocketAddr,
        _worker_id: Option<usize>,
    ) {
    }
}

#[cfg(unix)]
impl ReceiveUnixDatagramServer for DummyImporter {
    fn receive_unix_packet(&self, _packet: &[u8], _peer_addr: UnixSocketAddr) {}
}

impl Importer for DummyImporter {
    fn collector(&self) -> &NodeName {
        Default::default()
    }
}
