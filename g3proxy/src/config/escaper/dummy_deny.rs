/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_types::metrics::{MetricTagMap, NodeName};
use g3_yaml::YamlDocPosition;

use super::{EscaperConfig, EscaperConfigDiffAction};
use crate::config::escaper::AnyEscaperConfig;

const ESCAPER_CONFIG_DEFAULT_TYPE: &str = "DummyDeny";

#[derive(Clone)]
pub(crate) struct DummyDenyEscaperConfig {
    pub(crate) name: NodeName,
    position: Option<YamlDocPosition>,
    custom_type: String,
    pub(crate) extra_metrics_tags: Option<Arc<MetricTagMap>>,
}

impl DummyDenyEscaperConfig {
    pub(crate) fn new(position: Option<YamlDocPosition>, custom_type: Option<&str>) -> Self {
        DummyDenyEscaperConfig {
            name: NodeName::default(),
            position,
            custom_type: match custom_type {
                Some(custom_type) => custom_type.to_string(),
                None => ESCAPER_CONFIG_DEFAULT_TYPE.to_string(),
            },
            extra_metrics_tags: None,
        }
    }

    pub(super) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
        custom_type: Option<&str>,
    ) -> anyhow::Result<Self> {
        let mut escaper = Self::new(position, custom_type);
        g3_yaml::foreach_kv(map, |k, v| escaper.set(k, v))?;
        escaper.check()?;
        Ok(escaper)
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        Ok(())
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match k {
            super::CONFIG_KEY_ESCAPER_TYPE => Ok(()),
            super::CONFIG_KEY_ESCAPER_NAME => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "extra_metrics_tags" => {
                let tags = g3_yaml::value::as_static_metrics_tags(v)
                    .context(format!("invalid static metrics tags value for key {k}"))?;
                self.extra_metrics_tags = Some(Arc::new(tags));
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}

impl EscaperConfig for DummyDenyEscaperConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn r#type(&self) -> &str {
        &self.custom_type
    }

    fn resolver(&self) -> &NodeName {
        Default::default()
    }

    fn diff_action(&self, new: &AnyEscaperConfig) -> EscaperConfigDiffAction {
        let AnyEscaperConfig::DummyDeny(_new) = new else {
            return EscaperConfigDiffAction::SpawnNew;
        };

        EscaperConfigDiffAction::NoAction
    }
}
