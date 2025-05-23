/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
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
    sender: mpsc::UnboundedSender<(DateTime<Utc>, MetricRecord)>,
}

impl GraphiteExporter {
    fn new(config: GraphiteExporterConfig) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let (agg_sender, agg_receiver) = mpsc::unbounded_channel();
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
        let _ = self.sender.send((time, record.clone())); // TODO record drop
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
