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
use crate::config::exporter::graphite::GraphiteExporterConfig;
use crate::config::exporter::{AnyExporterConfig, ExporterConfig};
use crate::runtime::export::{AggregateExportRuntime, StreamExportRuntime};
use crate::types::MetricRecord;

mod format;
use format::{GraphitePlaintextAggregateExport, GraphitePlaintextStreamExport};

pub(crate) struct GraphiteExporter {
    config: GraphiteExporterConfig,
    sender: mpsc::Sender<(DateTime<Utc>, MetricRecord)>,
}

impl GraphiteExporter {
    fn new(config: GraphiteExporterConfig) -> Self {
        let (sender, receiver) = mpsc::channel(1024);
        let (agg_sender, agg_receiver) = mpsc::channel(1024);
        let aggregate_export = GraphitePlaintextAggregateExport::new(&config, agg_sender);
        let aggregate_runtime = AggregateExportRuntime::new(aggregate_export, receiver);

        let http_export = GraphitePlaintextStreamExport::default();
        let http_runtime =
            StreamExportRuntime::new(config.stream_export.clone(), http_export, agg_receiver);

        tokio::spawn(async move { aggregate_runtime.into_running().await });
        tokio::spawn(http_runtime.into_running());
        GraphiteExporter { config, sender }
    }

    pub(crate) fn prepare_initial(config: GraphiteExporterConfig) -> ArcExporterInternal {
        let server = GraphiteExporter::new(config);
        Arc::new(server)
    }

    fn prepare_reload(&self, config: AnyExporterConfig) -> anyhow::Result<GraphiteExporter> {
        if let AnyExporterConfig::Graphite(config) = config {
            Ok(GraphiteExporter::new(config))
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.exporter_type(),
                config.exporter_type()
            ))
        }
    }
}

impl Exporter for GraphiteExporter {
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

impl ExporterInternal for GraphiteExporter {
    fn _clone_config(&self) -> AnyExporterConfig {
        AnyExporterConfig::Graphite(self.config.clone())
    }

    fn _reload(&self, config: AnyExporterConfig) -> anyhow::Result<ArcExporterInternal> {
        let exporter = self.prepare_reload(config)?;
        Ok(Arc::new(exporter))
    }
}
