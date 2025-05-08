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

use std::time::Duration;

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use super::{AnyExporterConfig, ExporterConfig, ExporterConfigDiffAction};
use crate::runtime::export::StreamExportConfig;

const EXPORTER_CONFIG_TYPE: &str = "Graphite";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GraphiteExporterConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) emit_interval: Duration,
    pub(crate) stream_export: StreamExportConfig,
}

impl GraphiteExporterConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        GraphiteExporterConfig {
            name: NodeName::default(),
            position,
            emit_interval: Duration::from_secs(10),
            stream_export: StreamExportConfig::new(2003),
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut collector = GraphiteExporterConfig::new(position);

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
            "emit_interval" => {
                self.emit_interval = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            _ => self.stream_export.set_by_yaml_kv(k, v),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        self.stream_export.check(self.name.clone())?;
        Ok(())
    }
}

impl ExporterConfig for GraphiteExporterConfig {
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
        let AnyExporterConfig::Graphite(_new) = new else {
            return ExporterConfigDiffAction::SpawnNew;
        };

        ExporterConfigDiffAction::Reload
    }
}
