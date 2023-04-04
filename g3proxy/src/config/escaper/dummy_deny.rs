/*
 * Copyright 2023 ByteDance and/or its affiliates.
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

use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_yaml::YamlDocPosition;

use super::{EscaperConfig, EscaperConfigDiffAction};
use crate::config::escaper::AnyEscaperConfig;

const ESCAPER_CONFIG_DEFAULT_TYPE: &str = "DummyDeny";

#[derive(Clone)]
pub(crate) struct DummyDenyEscaperConfig {
    pub(crate) name: MetricsName,
    position: Option<YamlDocPosition>,
    custom_type: String,
    pub(crate) extra_metrics_tags: Option<Arc<StaticMetricsTags>>,
}

impl DummyDenyEscaperConfig {
    pub(crate) fn new(position: Option<YamlDocPosition>, custom_type: Option<&str>) -> Self {
        DummyDenyEscaperConfig {
            name: MetricsName::default(),
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
                self.name = g3_yaml::value::as_metrics_name(v)?;
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
    fn name(&self) -> &MetricsName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn escaper_type(&self) -> &str {
        &self.custom_type
    }

    fn resolver(&self) -> &MetricsName {
        Default::default()
    }

    fn diff_action(&self, new: &AnyEscaperConfig) -> EscaperConfigDiffAction {
        let _ = match new {
            AnyEscaperConfig::DummyDeny(config) => config,
            _ => return EscaperConfigDiffAction::SpawnNew,
        };

        EscaperConfigDiffAction::NoAction
    }
}
