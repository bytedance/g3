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
use crate::config::exporter::influxdb::InfluxdbV3ExporterConfig;
use crate::config::exporter::{AnyExporterConfig, ExporterConfig};
use crate::runtime::export::{AggregateExportRuntime, HttpExportRuntime};
use crate::types::MetricRecord;

use super::{InfluxdbAggregateExport, InfluxdbHttpExport};

pub(crate) struct InfluxdbV3Exporter {
    config: InfluxdbV3ExporterConfig,
    sender: mpsc::Sender<(DateTime<Utc>, MetricRecord)>,
}

impl InfluxdbV3Exporter {
    fn new(config: InfluxdbV3ExporterConfig) -> anyhow::Result<Self> {
        let (sender, receiver) = mpsc::channel(1024);
        let (agg_sender, agg_receiver) = mpsc::channel(1024);
        let aggregate_export = InfluxdbAggregateExport::new(&config, agg_sender);
        let aggregate_runtime = AggregateExportRuntime::new(aggregate_export, receiver);

        let http_export = InfluxdbHttpExport::new(&config)?;
        let http_runtime =
            HttpExportRuntime::new(config.http_export.clone(), http_export, agg_receiver);

        tokio::spawn(async move { aggregate_runtime.into_running().await });
        tokio::spawn(http_runtime.into_running());
        Ok(InfluxdbV3Exporter { config, sender })
    }

    pub(crate) fn prepare_initial(
        config: InfluxdbV3ExporterConfig,
    ) -> anyhow::Result<ArcExporterInternal> {
        let server = InfluxdbV3Exporter::new(config)?;
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyExporterConfig) -> anyhow::Result<InfluxdbV3Exporter> {
        if let AnyExporterConfig::InfluxdbV3(config) = config {
            InfluxdbV3Exporter::new(config)
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.exporter_type(),
                config.exporter_type()
            ))
        }
    }
}

impl Exporter for InfluxdbV3Exporter {
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

impl ExporterInternal for InfluxdbV3Exporter {
    fn _clone_config(&self) -> AnyExporterConfig {
        AnyExporterConfig::InfluxdbV3(self.config.clone())
    }

    fn _reload(&self, config: AnyExporterConfig) -> anyhow::Result<ArcExporterInternal> {
        let exporter = self.prepare_reload(config)?;
        Ok(Arc::new(exporter))
    }
}
