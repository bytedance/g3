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

use anyhow::{anyhow, Context};
use yaml_rust::{yaml, Yaml};

use g3_types::collection::{SelectivePickPolicy, WeightedValue};
use g3_types::metrics::MetricsName;
use g3_yaml::YamlDocPosition;

use super::{AnyEscaperConfig, EscaperConfig, EscaperConfigDiffAction};

const ESCAPER_CONFIG_TYPE: &str = "RouteSelect";

#[derive(Clone, PartialEq)]
pub(crate) struct RouteSelectEscaperConfig {
    pub(crate) name: MetricsName,
    position: Option<YamlDocPosition>,
    pub(crate) next_nodes: Vec<WeightedValue<MetricsName>>,
    pub(crate) next_pick_policy: SelectivePickPolicy,
}

impl RouteSelectEscaperConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        RouteSelectEscaperConfig {
            name: MetricsName::default(),
            position,
            next_nodes: Vec::new(),
            next_pick_policy: SelectivePickPolicy::Rendezvous,
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
            "next_nodes" => match v {
                Yaml::String(_) => {
                    let item = g3_yaml::value::as_weighted_metrics_name(v)
                        .context(format!("invalid weighted metrics name value for key {k}"))?;
                    self.next_nodes.push(item);
                    Ok(())
                }
                Yaml::Array(seq) => {
                    for (i, v) in seq.iter().enumerate() {
                        let item = g3_yaml::value::as_weighted_metrics_name(v)
                            .context(format!("invalid weighted metrics name value for {k}#{i}"))?;
                        self.next_nodes.push(item);
                    }
                    Ok(())
                }
                _ => Err(anyhow!("invalid value type for key {k}")),
            },
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
    fn name(&self) -> &MetricsName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn escaper_type(&self) -> &str {
        ESCAPER_CONFIG_TYPE
    }

    fn resolver(&self) -> &MetricsName {
        Default::default()
    }

    fn diff_action(&self, new: &AnyEscaperConfig) -> EscaperConfigDiffAction {
        let new = match new {
            AnyEscaperConfig::RouteSelect(config) => config,
            _ => return EscaperConfigDiffAction::SpawnNew,
        };

        if self.eq(new) {
            return EscaperConfigDiffAction::NoAction;
        }

        EscaperConfigDiffAction::Reload
    }

    fn dependent_escaper(&self) -> Option<BTreeSet<MetricsName>> {
        let mut set = BTreeSet::new();
        for v in &self.next_nodes {
            set.insert(v.inner().clone());
        }
        Some(set)
    }
}
