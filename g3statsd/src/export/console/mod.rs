/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use chrono::{DateTime, Utc};

use g3_types::metrics::NodeName;

use super::{ArcExporterInternal, Exporter, ExporterInternal};
use crate::config::exporter::console::ConsoleExporterConfig;
use crate::config::exporter::{AnyExporterConfig, ExporterConfig};
use crate::types::MetricRecord;

pub(crate) struct ConsoleExporter {
    config: ConsoleExporterConfig,
}

impl ConsoleExporter {
    fn new(config: ConsoleExporterConfig) -> Self {
        ConsoleExporter { config }
    }

    pub(crate) fn prepare_initial(config: ConsoleExporterConfig) -> ArcExporterInternal {
        let server = ConsoleExporter::new(config);
        Arc::new(server)
    }

    fn prepare_reload(&self, config: AnyExporterConfig) -> anyhow::Result<ConsoleExporter> {
        if let AnyExporterConfig::Console(config) = config {
            Ok(ConsoleExporter::new(config))
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.exporter_type(),
                config.exporter_type()
            ))
        }
    }
}

impl Exporter for ConsoleExporter {
    #[inline]
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    #[inline]
    fn r#type(&self) -> &'static str {
        self.config.exporter_type()
    }

    fn add_metric(&self, time: DateTime<Utc>, record: &MetricRecord) {
        println!(
            "{time} {} {} {}",
            record.name.display('.'),
            record.value,
            record.tag_map.display_opentsdb(),
        );
    }
}

impl ExporterInternal for ConsoleExporter {
    fn _clone_config(&self) -> AnyExporterConfig {
        AnyExporterConfig::Console(self.config.clone())
    }

    fn _reload(&self, config: AnyExporterConfig) -> anyhow::Result<ArcExporterInternal> {
        let exporter = self.prepare_reload(config)?;
        Ok(Arc::new(exporter))
    }
}
