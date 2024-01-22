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

use anyhow::anyhow;
use yaml_rust::{yaml, Yaml};

use g3_types::metrics::MetricsName;
use g3_yaml::YamlDocPosition;

use super::{AnyBackendConfig, BackendConfig, BackendConfigDiffAction};

const BACKEND_CONFIG_TYPE: &str = "DummyClose";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DummyCloseBackendConfig {
    name: MetricsName,
    position: Option<YamlDocPosition>,
}

impl DummyCloseBackendConfig {
    pub(crate) fn new(name: &MetricsName, position: Option<YamlDocPosition>) -> Self {
        DummyCloseBackendConfig {
            name: name.clone(),
            position,
        }
    }

    pub(super) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut server = DummyCloseBackendConfig {
            name: MetricsName::default(),
            position,
        };
        g3_yaml::foreach_kv(map, |k, v| server.set(k, v))?;
        server.check()?;
        Ok(server)
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
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
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}

impl BackendConfig for DummyCloseBackendConfig {
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
        let _ = match new {
            AnyBackendConfig::DummyClose(config) => config,
            _ => return BackendConfigDiffAction::SpawnNew,
        };

        BackendConfigDiffAction::NoAction
    }
}
