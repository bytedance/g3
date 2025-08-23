/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use chrono::{DateTime, Utc};

use g3_types::metrics::NodeName;

use super::{ArcExporterInternal, Exporter, ExporterInternal};
use crate::config::exporter::memory::MemoryExporterConfig;
use crate::config::exporter::{AnyExporterConfig, ExporterConfig};
use crate::types::MetricRecord;

mod store;
use store::MemoryStore;

pub(crate) struct MemoryExporter {
    config: MemoryExporterConfig,
    store: Arc<MemoryStore>,
}

impl MemoryExporter {
    fn new(config: MemoryExporterConfig, store: Arc<MemoryStore>) -> Self {
        MemoryExporter { config, store }
    }

    pub(crate) fn prepare_initial(config: MemoryExporterConfig) -> ArcExporterInternal {
        let store = MemoryStore::default();
        let server = MemoryExporter::new(config, Arc::new(store));
        Arc::new(server)
    }

    fn prepare_reload(&self, config: AnyExporterConfig) -> anyhow::Result<MemoryExporter> {
        if let AnyExporterConfig::Memory(config) = config {
            let store = self.store.clone();
            Ok(MemoryExporter::new(config, store))
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.exporter_type(),
                config.exporter_type()
            ))
        }
    }
}

impl Exporter for MemoryExporter {
    #[inline]
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    #[inline]
    fn r#type(&self) -> &'static str {
        self.config.exporter_type()
    }

    fn add_metric(&self, time: DateTime<Utc>, record: &MetricRecord) {
        self.store
            .add_record(time, self.config.store_count.get(), record);
    }
}

impl ExporterInternal for MemoryExporter {
    fn _clone_config(&self) -> AnyExporterConfig {
        AnyExporterConfig::Memory(self.config.clone())
    }

    fn _reload(&self, config: AnyExporterConfig) -> anyhow::Result<ArcExporterInternal> {
        let exporter = self.prepare_reload(config)?;
        Ok(Arc::new(exporter))
    }
}
