/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, anyhow};
use http::uri::PathAndQuery;
use yaml_rust::{Yaml, yaml};

use g3_types::metrics::{MetricTagMap, NodeName};
use g3_yaml::YamlDocPosition;

use super::{AnyExporterConfig, ExporterConfig, ExporterConfigDiffAction};
use crate::runtime::export::HttpExportConfig;
use crate::types::MetricName;

const EXPORTER_CONFIG_TYPE: &str = "OpenTSDB";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OpentsdbExporterConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) emit_interval: Duration,
    pub(crate) max_data_points: usize,
    pub(crate) http_export: HttpExportConfig,
    sync_timeout: Option<Duration>,
    pub(crate) prefix: Option<MetricName>,
    pub(crate) global_tags: MetricTagMap,
}

impl OpentsdbExporterConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        OpentsdbExporterConfig {
            name: NodeName::default(),
            position,
            emit_interval: Duration::from_secs(10),
            max_data_points: 50,
            http_export: HttpExportConfig::new(4242),
            sync_timeout: None,
            prefix: None,
            global_tags: MetricTagMap::default(),
        }
    }

    pub(crate) fn build_api_path(&self) -> anyhow::Result<PathAndQuery> {
        match self.sync_timeout {
            Some(Duration::ZERO) => Ok(PathAndQuery::from_static("/api/put?sync")),
            Some(dur) => {
                let timeout_ms = dur.as_millis().max(1);
                let path = format!("/api/put?sync&sync_timeout={timeout_ms}");
                PathAndQuery::from_str(&path)
                    .map_err(|e| anyhow!("invalid opentsdb api path {path}: {e}"))
            }
            None => Ok(PathAndQuery::from_static("/api/put")),
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut collector = OpentsdbExporterConfig::new(position);

        g3_yaml::foreach_kv(map, |k, v| collector.set(k, v))?;

        collector.check()?;
        Ok(collector)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_EXPORTER_TYPE => Ok(()),
            super::CONFIG_KEY_EXPORTER_NAME => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "sync_timeout" => {
                let timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                self.sync_timeout = Some(timeout);
                Ok(())
            }
            "emit_interval" => {
                self.emit_interval = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "max_data_points" => {
                self.max_data_points = g3_yaml::value::as_usize(v)?;
                Ok(())
            }
            "prefix" => {
                let prefix = MetricName::parse_yaml(v)
                    .context(format!("invalid metric name value for key {k}"))?;
                self.prefix = Some(prefix);
                Ok(())
            }
            "global_tags" => {
                self.global_tags = g3_yaml::value::as_static_metrics_tags(v)
                    .context(format!("invalid static metrics tags value for key {k}"))?;
                Ok(())
            }
            _ => self.http_export.set_by_yaml_kv(k, v),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        self.http_export.check(self.name.clone())?;
        Ok(())
    }
}

impl ExporterConfig for OpentsdbExporterConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn exporter_type(&self) -> &'static str {
        EXPORTER_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyExporterConfig) -> ExporterConfigDiffAction {
        let AnyExporterConfig::Opentsdb(_new) = new else {
            return ExporterConfigDiffAction::SpawnNew;
        };

        ExporterConfigDiffAction::Reload
    }
}
