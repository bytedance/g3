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

use anyhow::anyhow;
use yaml_rust::{Yaml, yaml};

use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use super::{AnyCollectConfig, CollectConfig, CollectConfigDiffAction};

const COLLECT_CONFIG_TYPE: &str = "Dummy";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DummyCollectConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
}

impl DummyCollectConfig {
    pub(crate) fn with_name(name: &NodeName, position: Option<YamlDocPosition>) -> Self {
        DummyCollectConfig {
            name: name.clone(),
            position,
        }
    }

    fn new(position: Option<YamlDocPosition>) -> Self {
        DummyCollectConfig {
            name: NodeName::default(),
            position,
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut input = DummyCollectConfig::new(position);

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

impl CollectConfig for DummyCollectConfig {
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
        let AnyCollectConfig::Dummy(_new) = new else {
            return CollectConfigDiffAction::SpawnNew;
        };

        CollectConfigDiffAction::NoAction
    }
}
