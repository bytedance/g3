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

use std::collections::BTreeSet;
use std::time::Duration;

use anyhow::anyhow;
use yaml_rust::{Yaml, yaml};

use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use super::{AnyEscaperConfig, EscaperConfig, EscaperConfigDiffAction};

const ESCAPER_CONFIG_TYPE: &str = "RouteFailover";

#[derive(Clone, PartialEq)]
pub(crate) struct RouteFailoverEscaperConfig {
    pub(crate) name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) primary_node: NodeName,
    pub(crate) standby_node: NodeName,
    pub(crate) fallback_delay: Duration,
}

impl RouteFailoverEscaperConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        RouteFailoverEscaperConfig {
            name: NodeName::default(),
            position,
            primary_node: NodeName::default(),
            standby_node: NodeName::default(),
            fallback_delay: Duration::from_millis(100),
        }
    }

    pub(super) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut config = Self::new(position);

        g3_yaml::foreach_kv(map, |k, v| config.set(k, v))?;

        config.check()?;
        Ok(config)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_ESCAPER_TYPE => Ok(()),
            super::CONFIG_KEY_ESCAPER_NAME => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "primary" | "primary_next" => {
                self.primary_node = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "standby" | "standby_next" => {
                self.standby_node = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "fallback_delay" | "delay" | "fallback_timeout" | "timeout" => {
                self.fallback_delay = g3_yaml::humanize::as_duration(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.primary_node.is_empty() {
            return Err(anyhow!("no primary next escaper set"));
        }
        if self.standby_node.is_empty() {
            return Err(anyhow!("no standby next escaper set"));
        }

        Ok(())
    }
}

impl EscaperConfig for RouteFailoverEscaperConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn r#type(&self) -> &str {
        ESCAPER_CONFIG_TYPE
    }

    fn resolver(&self) -> &NodeName {
        Default::default()
    }

    fn diff_action(&self, new: &AnyEscaperConfig) -> EscaperConfigDiffAction {
        let AnyEscaperConfig::RouteFailover(new) = new else {
            return EscaperConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            return EscaperConfigDiffAction::NoAction;
        }

        EscaperConfigDiffAction::Reload
    }

    fn dependent_escaper(&self) -> Option<BTreeSet<NodeName>> {
        let mut set = BTreeSet::new();
        set.insert(self.primary_node.clone());
        set.insert(self.standby_node.clone());
        Some(set)
    }
}
