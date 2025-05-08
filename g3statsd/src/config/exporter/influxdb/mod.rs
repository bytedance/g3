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

use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, anyhow};
use http::uri::PathAndQuery;
use yaml_rust::{Yaml, yaml};

use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use super::{AnyExporterConfig, ExporterConfig, ExporterConfigDiffAction};
use crate::runtime::export::HttpExportConfig;

mod precision;
pub(crate) use precision::TimestampPrecision;

mod version;
use version::ApiVersion;

const EXPORTER_CONFIG_TYPE: &str = "InfluxDB";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InfluxdbExporterConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) emit_interval: Duration,
    pub(crate) max_body_lines: usize,
    pub(crate) http_export: HttpExportConfig,
    version: ApiVersion,
    database: String,
    pub(crate) precision: TimestampPrecision,
    v3_no_sync: bool,
}

impl InfluxdbExporterConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        InfluxdbExporterConfig {
            name: NodeName::default(),
            position,
            emit_interval: Duration::from_secs(10),
            max_body_lines: 10000,
            http_export: HttpExportConfig::new(8181),
            version: ApiVersion::V2,
            database: String::new(),
            precision: TimestampPrecision::Seconds,
            v3_no_sync: false,
        }
    }

    pub(crate) fn build_api_path(&self) -> anyhow::Result<PathAndQuery> {
        let path = match self.version {
            ApiVersion::V1 => {
                format!(
                    "/write?db={}&precision={}",
                    self.database,
                    self.precision.query_value()
                )
            }
            ApiVersion::V2 => {
                format!(
                    "/api/v2/write?bucket={}&precision={}",
                    self.database,
                    self.precision.query_value()
                )
            }
            ApiVersion::V3 => {
                if self.v3_no_sync {
                    format!(
                        "/api/v3/write_lp?db={}&precision={}&no_sync=true",
                        self.database,
                        self.precision.query_value()
                    )
                } else {
                    format!(
                        "/api/v3/write_lp?db={}&precision={}",
                        self.database,
                        self.precision.query_value()
                    )
                }
            }
        };
        PathAndQuery::from_str(&path).map_err(|e| anyhow!("invalid influxdb api path {path}: {e}"))
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut collector = InfluxdbExporterConfig::new(position);

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
            "api_version" | "version" => {
                self.version = ApiVersion::parse_yaml(v)
                    .context(format!("invalid influxdb api version value for key {k}"))?;
                Ok(())
            }
            "database" => {
                self.database = g3_yaml::value::as_string(v)?;
                Ok(())
            }
            "precision" => {
                self.precision = TimestampPrecision::parse_yaml(v)
                    .context(format!("invalid timestamp precision value for key {k}"))?;
                Ok(())
            }
            "v3_no_sync" => {
                self.v3_no_sync = g3_yaml::value::as_bool(v)?;
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
            _ => self.http_export.set_by_yaml_kv(k, v),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.database.is_empty() {
            return Err(anyhow!("database is not set"));
        }
        self.http_export.check(self.name.clone())?;
        Ok(())
    }
}

impl ExporterConfig for InfluxdbExporterConfig {
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
        let AnyExporterConfig::Influxdb(_new) = new else {
            return ExporterConfigDiffAction::SpawnNew;
        };

        ExporterConfigDiffAction::Reload
    }
}
