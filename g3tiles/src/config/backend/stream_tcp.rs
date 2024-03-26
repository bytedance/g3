/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use anyhow::{anyhow, Context};
use yaml_rust::{yaml, Yaml};

use g3_histogram::HistogramMetricsConfig;
use g3_types::collection::SelectivePickPolicy;
use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_yaml::YamlDocPosition;

use super::{AnyBackendConfig, BackendConfig, BackendConfigDiffAction};
use crate::config::discover::DiscoverRegisterData;

const BACKEND_CONFIG_TYPE: &str = "StreamTcp";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StreamTcpBackendConfig {
    name: MetricsName,
    position: Option<YamlDocPosition>,
    pub(crate) discover: MetricsName,
    pub(crate) discover_data: DiscoverRegisterData,
    pub(crate) peer_pick_policy: SelectivePickPolicy,
    pub(crate) extra_metrics_tags: Option<Arc<StaticMetricsTags>>,
    pub(crate) duration_stats: HistogramMetricsConfig,
}

impl StreamTcpBackendConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        StreamTcpBackendConfig {
            name: MetricsName::default(),
            position,
            discover: MetricsName::default(),
            discover_data: DiscoverRegisterData::Null,
            peer_pick_policy: SelectivePickPolicy::Random,
            extra_metrics_tags: None,
            duration_stats: HistogramMetricsConfig::default(),
        }
    }

    pub(super) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut connector = StreamTcpBackendConfig::new(position);
        g3_yaml::foreach_kv(map, |k, v| connector.set(k, v))?;
        connector.check()?;
        Ok(connector)
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.discover.is_empty() {
            return Err(anyhow!("no discover set"));
        }
        if matches!(self.discover_data, DiscoverRegisterData::Null) {
            return Err(anyhow!("no discover data set"));
        }
        Ok(())
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match k {
            super::CONFIG_KEY_BACKEND_TYPE => Ok(()),
            super::CONFIG_KEY_BACKEND_NAME => {
                self.name = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "discover" => {
                self.discover = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "discover_data" => {
                self.discover_data = DiscoverRegisterData::Yaml(v.clone());
                Ok(())
            }
            "peer_pick_policy" => {
                self.peer_pick_policy = g3_yaml::value::as_selective_pick_policy(v)?;
                Ok(())
            }
            "extra_metrics_tags" => {
                let tags = g3_yaml::value::as_static_metrics_tags(v)
                    .context(format!("invalid static metrics tags value for key {k}"))?;
                self.extra_metrics_tags = Some(Arc::new(tags));
                Ok(())
            }
            "duration_stats" | "duration_metrics" => {
                self.duration_stats = g3_yaml::value::as_histogram_metrics_config(v).context(
                    format!("invalid histogram metrics config value for key {k}"),
                )?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}

impl BackendConfig for StreamTcpBackendConfig {
    fn name(&self) -> &MetricsName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn backend_type(&self) -> &'static str {
        BACKEND_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyBackendConfig) -> BackendConfigDiffAction {
        let AnyBackendConfig::StreamTcp(new) = new else {
            return BackendConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            return BackendConfigDiffAction::NoAction;
        }

        BackendConfigDiffAction::Reload
    }
}
