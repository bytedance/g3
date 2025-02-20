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
use g3_types::metrics::NodeName;
use indexmap::IndexSet;
use yaml_rust::{Yaml, yaml};

use g3_yaml::YamlDocPosition;

use super::{AnyEscaperConfig, EscaperConfig, EscaperConfigDiffAction};

const ESCAPER_CONFIG_TYPE: &str = "RouteMapping";

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct RouteMappingEscaperConfig {
    pub(crate) name: NodeName,
    position: Option<YamlDocPosition>,
    // no duplication for next escapers, and the order is important
    pub(crate) next_nodes: IndexSet<NodeName>,
}

impl RouteMappingEscaperConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        RouteMappingEscaperConfig {
            name: NodeName::default(),
            position,
            next_nodes: IndexSet::new(),
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
                self.name = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "next" => {
                if let Yaml::Array(seq) = v {
                    for (i, escaper) in seq.iter().enumerate() {
                        let name = g3_yaml::value::as_metrics_name(escaper)
                            .context(format!("invalid metrics name value for {k}#{i}"))?;
                        // duplicate values should report an error
                        if !self.next_nodes.insert(name.clone()) {
                            return Err(anyhow!("found duplicate next node: {name}"));
                        }
                    }
                    Ok(())
                } else {
                    Err(anyhow!("invalid array value for key {k}"))
                }
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.next_nodes.is_empty() {
            return Err(anyhow!("no next escaper found"));
        }

        Ok(())
    }
}

impl EscaperConfig for RouteMappingEscaperConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn escaper_type(&self) -> &str {
        ESCAPER_CONFIG_TYPE
    }

    fn resolver(&self) -> &NodeName {
        Default::default()
    }

    fn diff_action(&self, new: &AnyEscaperConfig) -> EscaperConfigDiffAction {
        let AnyEscaperConfig::RouteMapping(new) = new else {
            return EscaperConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            return EscaperConfigDiffAction::NoAction;
        }

        EscaperConfigDiffAction::Reload
    }

    fn dependent_escaper(&self) -> Option<BTreeSet<NodeName>> {
        let mut set = BTreeSet::new();
        for name in &self.next_nodes {
            set.insert(name.clone());
        }
        Some(set)
    }
}
