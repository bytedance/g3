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

use super::{AnyImporterConfig, ImporterConfig, ImporterConfigDiffAction};

const IMPORTER_CONFIG_TYPE: &str = "Dummy";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DummyImporterConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
}

impl DummyImporterConfig {
    pub(crate) fn with_name(name: &NodeName, position: Option<YamlDocPosition>) -> Self {
        DummyImporterConfig {
            name: name.clone(),
            position,
        }
    }

    fn new(position: Option<YamlDocPosition>) -> Self {
        DummyImporterConfig {
            name: NodeName::default(),
            position,
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut importer = DummyImporterConfig::new(position);

        g3_yaml::foreach_kv(map, |k, v| importer.set(k, v))?;

        importer.check()?;
        Ok(importer)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_IMPORTER_TYPE => Ok(()),
            super::CONFIG_KEY_IMPORTER_NAME => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
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

impl ImporterConfig for DummyImporterConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn importer_type(&self) -> &'static str {
        IMPORTER_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyImporterConfig) -> ImporterConfigDiffAction {
        let AnyImporterConfig::Dummy(_new) = new else {
            return ImporterConfigDiffAction::SpawnNew;
        };

        ImporterConfigDiffAction::NoAction
    }

    fn collector(&self) -> &NodeName {
        Default::default()
    }
}
