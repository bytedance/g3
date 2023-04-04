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

use anyhow::anyhow;
use g3_types::metrics::MetricsName;
use yaml_rust::{yaml, Yaml};

use g3_yaml::YamlDocPosition;

use super::{AnyEscaperConfig, EscaperConfig, EscaperConfigDiffAction};

const ESCAPER_CONFIG_TYPE: &str = "TrickFloat";

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct TrickFloatEscaperConfig {
    pub(crate) name: String,
    position: Option<YamlDocPosition>,
    pub(crate) next_nodes: BTreeSet<String>, // no duplication for next escapers
}

impl TrickFloatEscaperConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        TrickFloatEscaperConfig {
            name: String::new(),
            position,
            next_nodes: BTreeSet::new(),
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
                if let Yaml::String(name) = v {
                    self.name.clone_from(name);
                    Ok(())
                } else {
                    Err(anyhow!("invalid string value for key {k}"))
                }
            }
            "next" => {
                if let Yaml::Array(seq) = v {
                    for (i, escaper) in seq.iter().enumerate() {
                        match escaper {
                            Yaml::String(s) => {
                                if s.is_empty() {
                                    return Err(anyhow!("empty string value for {k}#{i}"));
                                }
                                // duplicate values won't report an error
                                self.next_nodes.insert(s.to_string());
                            }
                            _ => return Err(anyhow!("invalid value type for {k}#{i}")),
                        };
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

impl EscaperConfig for TrickFloatEscaperConfig {
    fn name(&self) -> &str {
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
            AnyEscaperConfig::TrickFloat(config) => config,
            _ => return EscaperConfigDiffAction::SpawnNew,
        };

        if self.eq(new) {
            return EscaperConfigDiffAction::NoAction;
        }

        EscaperConfigDiffAction::Reload
    }

    fn dependent_escaper(&self) -> Option<BTreeSet<String>> {
        Some(self.next_nodes.clone())
    }
}
