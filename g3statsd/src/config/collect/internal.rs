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

use super::{AnyCollectConfig, CollectConfig, CollectConfigDiffAction};

const COLLECT_CONFIG_TYPE: &str = "Internal";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InternalCollectConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    emit_interval: Duration,
}

impl InternalCollectConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        InternalCollectConfig {
            name: NodeName::default(),
            position,
            emit_interval: Duration::from_secs(1),
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut input = InternalCollectConfig::new(position);

        g3_yaml::foreach_kv(map, |k, v| input.set(k, v))?;

        input.check()?;
        Ok(input)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_COLLECT_TYPE => Ok(()),
            super::CONFIG_KEY_COLLECT_NAME => {
                self.name = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "emit_interval" => {
                self.emit_interval = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        Ok(())
    }
}

impl CollectConfig for InternalCollectConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn collect_type(&self) -> &'static str {
        COLLECT_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyCollectConfig) -> CollectConfigDiffAction {
        let AnyCollectConfig::Internal(_new) = new else {
            return CollectConfigDiffAction::SpawnNew;
        };

        CollectConfigDiffAction::ReloadOnlyConfig
    }
}
