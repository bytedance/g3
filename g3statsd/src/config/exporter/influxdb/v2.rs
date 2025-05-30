/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, anyhow};
use http::HeaderValue;
use http::uri::PathAndQuery;
use yaml_rust::{Yaml, yaml};

use g3_types::metrics::{MetricTagMap, NodeName};
use g3_yaml::YamlDocPosition;

use super::{
    AnyExporterConfig, ExporterConfig, ExporterConfigDiffAction, InfluxdbExporterConfig,
    TimestampPrecision,
};
use crate::runtime::export::HttpExportConfig;
use crate::types::MetricName;

const EXPORTER_CONFIG_TYPE: &str = "InfluxDB_V2";

const AUTH_TOKEN_ENV_VAR: &str = "INFLUX_TOKEN";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InfluxdbV2ExporterConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    emit_interval: Duration,
    max_body_lines: usize,
    pub(crate) http_export: HttpExportConfig,
    bucket: String,
    token: String,
    precision: TimestampPrecision,
    prefix: Option<MetricName>,
    global_tags: MetricTagMap,
}

impl InfluxdbV2ExporterConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        InfluxdbV2ExporterConfig {
            name: NodeName::default(),
            position,
            emit_interval: Duration::from_secs(10),
            max_body_lines: 10000,
            http_export: HttpExportConfig::new(8181),
            bucket: String::new(),
            token: String::new(),
            precision: TimestampPrecision::Seconds,
            prefix: None,
            global_tags: MetricTagMap::default(),
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut collector = InfluxdbV2ExporterConfig::new(position);

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
            "bucket" => {
                self.bucket = g3_yaml::value::as_string(v)?;
                Ok(())
            }
            "token" => {
                self.token = g3_yaml::value::as_http_header_value_string(v)
                    .context(format!("invalid http header value string for key {k}"))?;
                Ok(())
            }
            "precision" => {
                self.precision = TimestampPrecision::parse_yaml(v)
                    .context(format!("invalid timestamp precision value for key {k}"))?;
                Ok(())
            }
            "emit_interval" => {
                self.emit_interval = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "max_body_lines" => {
                self.max_body_lines = g3_yaml::value::as_usize(v)?;
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
        if self.bucket.is_empty() {
            return Err(anyhow!("database is not set"));
        }
        if self.token.is_empty() {
            if let Ok(token) = std::env::var(AUTH_TOKEN_ENV_VAR) {
                self.token = token;
            }
        }
        self.http_export.check(self.name.clone())?;
        Ok(())
    }
}

impl ExporterConfig for InfluxdbV2ExporterConfig {
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
        let AnyExporterConfig::InfluxdbV2(_new) = new else {
            return ExporterConfigDiffAction::SpawnNew;
        };

        ExporterConfigDiffAction::Reload
    }
}

impl InfluxdbExporterConfig for InfluxdbV2ExporterConfig {
    fn emit_interval(&self) -> Duration {
        self.emit_interval
    }

    fn precision(&self) -> TimestampPrecision {
        self.precision
    }

    fn max_body_lines(&self) -> usize {
        self.max_body_lines
    }

    fn prefix(&self) -> Option<MetricName> {
        self.prefix.clone()
    }

    fn global_tags(&self) -> MetricTagMap {
        self.global_tags.clone()
    }

    fn build_api_path(&self) -> anyhow::Result<PathAndQuery> {
        let path = format!(
            "/api/v2/write?bucket={}&precision={}",
            self.bucket,
            self.precision.v2_query_value()
        );
        PathAndQuery::from_str(&path).map_err(|e| anyhow!("invalid influxdb api path {path}: {e}"))
    }

    fn build_api_token(&self) -> Option<HeaderValue> {
        if self.token.is_empty() {
            return None;
        }
        let s = format!("Token {}", self.token);
        HeaderValue::from_str(&s).ok()
    }
}
