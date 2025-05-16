/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use yaml_rust::{Yaml, yaml};

use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use super::{AnyBackendConfig, BackendConfig, BackendConfigDiffAction};

const BACKEND_CONFIG_TYPE: &str = "DummyClose";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DummyCloseBackendConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
}

impl DummyCloseBackendConfig {
    pub(crate) fn new(name: &NodeName, position: Option<YamlDocPosition>) -> Self {
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
            name: NodeName::default(),
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
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}

impl BackendConfig for DummyCloseBackendConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn r#type(&self) -> &'static str {
        BACKEND_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyBackendConfig) -> BackendConfigDiffAction {
        let AnyBackendConfig::DummyClose(_new) = new else {
            return BackendConfigDiffAction::SpawnNew;
        };

        BackendConfigDiffAction::NoAction
    }
}
