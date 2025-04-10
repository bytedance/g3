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

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_types::collection::{SelectivePickPolicy, WeightedValue};
use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use super::{AnyEscaperConfig, EscaperConfig, EscaperConfigDiffAction};

const ESCAPER_CONFIG_TYPE: &str = "RouteSelect";

#[derive(Clone, PartialEq)]
pub(crate) struct RouteSelectEscaperConfig {
    pub(crate) name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) next_nodes: Vec<WeightedValue<NodeName>>,
    pub(crate) next_pick_policy: SelectivePickPolicy,
}

impl RouteSelectEscaperConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        RouteSelectEscaperConfig {
            name: NodeName::default(),
            position,
            next_nodes: Vec::new(),
            next_pick_policy: SelectivePickPolicy::Ketama,
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
            "next_nodes" => {
                self.next_nodes =
                    g3_yaml::value::as_list(v, g3_yaml::value::as_weighted_metric_node_name)
                        .context(format!(
                            "invalid weighted metrics name list value for key {k}"
                        ))?;
                Ok(())
            }
            "next_pick_policy" => {
                self.next_pick_policy = g3_yaml::value::as_selective_pick_policy(v)
                    .context(format!("invalid selective pick policy value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.next_nodes.is_empty() {
            return Err(anyhow!("no next escapers found"));
        }
        self.next_nodes.reverse(); // reverse as we push to the back

        Ok(())
    }
}

impl EscaperConfig for RouteSelectEscaperConfig {
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
        let AnyEscaperConfig::RouteSelect(new) = new else {
            return EscaperConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            return EscaperConfigDiffAction::NoAction;
        }

        EscaperConfigDiffAction::Reload
    }

    fn dependent_escaper(&self) -> Option<BTreeSet<NodeName>> {
        let mut set = BTreeSet::new();
        for v in &self.next_nodes {
            set.insert(v.inner().clone());
        }
        Some(set)
    }
}
