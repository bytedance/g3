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

use std::sync::Arc;

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use tokio::sync::mpsc;

use g3_types::metrics::NodeName;

use super::{ArcExporterInternal, Exporter, ExporterInternal};
use crate::config::exporter::opentsdb::OpentsdbExporterConfig;
use crate::config::exporter::{AnyExporterConfig, ExporterConfig};
use crate::types::MetricRecord;

mod format;
use format::OpentsdbHttpFormatter;

pub(crate) struct OpentsdbExporter {
    config: OpentsdbExporterConfig,
    sender: mpsc::Sender<(DateTime<Utc>, MetricRecord)>,
}

impl OpentsdbExporter {
    fn new(config: OpentsdbExporterConfig) -> anyhow::Result<Self> {
        let formatter = OpentsdbHttpFormatter::new(&config)?;
        let sender = config.http_export.spawn(formatter);
        Ok(OpentsdbExporter { config, sender })
    }

    pub(crate) fn prepare_initial(
        config: OpentsdbExporterConfig,
    ) -> anyhow::Result<ArcExporterInternal> {
        let server = OpentsdbExporter::new(config)?;
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyExporterConfig) -> anyhow::Result<OpentsdbExporter> {
        if let AnyExporterConfig::Opentsdb(config) = config {
            OpentsdbExporter::new(config)
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.exporter_type(),
                config.exporter_type()
            ))
        }
    }
}

impl Exporter for OpentsdbExporter {
    #[inline]
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    #[inline]
    fn r#type(&self) -> &'static str {
        self.config.exporter_type()
    }

    fn add_metric(&self, time: DateTime<Utc>, record: &MetricRecord) {
        let _ = self.sender.try_send((time, record.clone())); // TODO record drop
    }
}

impl ExporterInternal for OpentsdbExporter {
    fn _clone_config(&self) -> AnyExporterConfig {
        AnyExporterConfig::Opentsdb(self.config.clone())
    }

    fn _reload(&self, config: AnyExporterConfig) -> anyhow::Result<ArcExporterInternal> {
        let exporter = self.prepare_reload(config)?;
        Ok(Arc::new(exporter))
    }
}
