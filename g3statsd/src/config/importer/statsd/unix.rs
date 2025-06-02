/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::path::PathBuf;

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use super::{AnyImporterConfig, ImporterConfig, ImporterConfigDiffAction};

const IMPORTER_CONFIG_TYPE: &str = "StatsD_UNIX";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StatsdUnixImporterConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) collector: NodeName,
    pub(crate) listen: PathBuf,
}

impl StatsdUnixImporterConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        StatsdUnixImporterConfig {
            name: NodeName::default(),
            position,
            collector: Default::default(),
            listen: PathBuf::new(),
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut importer = StatsdUnixImporterConfig::new(position);

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
            "collector" => {
                self.collector = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "listen" => {
                self.listen = g3_yaml::value::as_absolute_path(v)
                    .context(format!("invalid unix listen path value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.collector.is_empty() {
            return Err(anyhow!("collector is not set"));
        }
        if self.listen.as_os_str().is_empty() {
            return Err(anyhow!("listen path is not set"));
        }

        Ok(())
    }
}

impl ImporterConfig for StatsdUnixImporterConfig {
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
        let AnyImporterConfig::StatsDUnix(new) = new else {
            return ImporterConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            return ImporterConfigDiffAction::NoAction;
        }

        if self.listen != new.listen {
            return ImporterConfigDiffAction::ReloadAndRespawn;
        }

        ImporterConfigDiffAction::ReloadNoRespawn
    }

    fn collector(&self) -> &NodeName {
        &self.collector
    }
}
