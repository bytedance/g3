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
use crate::config::exporter::influxdb::InfluxdbV2ExporterConfig;
use crate::config::exporter::{AnyExporterConfig, ExporterConfig};
use crate::runtime::export::{AggregateExportRuntime, HttpExportRuntime};
use crate::types::MetricRecord;

use super::{InfluxdbAggregateExport, InfluxdbHttpExport};

pub(crate) struct InfluxdbV2Exporter {
    config: InfluxdbV2ExporterConfig,
    sender: mpsc::UnboundedSender<(DateTime<Utc>, MetricRecord)>,
}

impl InfluxdbV2Exporter {
    fn new(config: InfluxdbV2ExporterConfig) -> anyhow::Result<Self> {
        let (sender, receiver) = mpsc::unbounded_channel();
        let (agg_sender, agg_receiver) = mpsc::unbounded_channel();
        let aggregate_export = InfluxdbAggregateExport::new(&config, agg_sender);
        let aggregate_runtime = AggregateExportRuntime::new(aggregate_export, receiver);

        let http_export = InfluxdbHttpExport::new(&config)?;
        let http_runtime =
            HttpExportRuntime::new(config.http_export.clone(), http_export, agg_receiver);

        tokio::spawn(async move { aggregate_runtime.into_running().await });
        tokio::spawn(http_runtime.into_running());
        Ok(InfluxdbV2Exporter { config, sender })
    }

    pub(crate) fn prepare_initial(
        config: InfluxdbV2ExporterConfig,
    ) -> anyhow::Result<ArcExporterInternal> {
        let server = InfluxdbV2Exporter::new(config)?;
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyExporterConfig) -> anyhow::Result<InfluxdbV2Exporter> {
        if let AnyExporterConfig::InfluxdbV2(config) = config {
            InfluxdbV2Exporter::new(config)
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.exporter_type(),
                config.exporter_type()
            ))
        }
    }
}

impl Exporter for InfluxdbV2Exporter {
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

impl ExporterInternal for InfluxdbV2Exporter {
    fn _clone_config(&self) -> AnyExporterConfig {
        AnyExporterConfig::InfluxdbV2(self.config.clone())
    }

    fn _reload(&self, config: AnyExporterConfig) -> anyhow::Result<ArcExporterInternal> {
        let exporter = self.prepare_reload(config)?;
        Ok(Arc::new(exporter))
    }
}
