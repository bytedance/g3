/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use super::{AnyEscaperConfig, EscaperConfig, EscaperConfigDiffAction};

mod child_match;
mod exact_match;
mod regex_match;
mod subnet_match;
mod suffix_match;

pub(crate) use child_match::{ChildMatch, ChildMatchBuilder};
pub(crate) use exact_match::{ExactMatch, ExactMatchBuilder};
pub(crate) use regex_match::{RegexMatch, RegexMatchBuilder};
pub(crate) use subnet_match::{SubnetMatch, SubnetMatchBuilder};
pub(crate) use suffix_match::{SuffixMatch, SuffixMatchBuilder};

const ESCAPER_CONFIG_TYPE: &str = "RouteUpstream";

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct RouteUpstreamEscaperConfig {
    pub(crate) name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) exact_match: ExactMatchBuilder,
    pub(crate) subnet_match: SubnetMatchBuilder,
    pub(crate) suffix_match: SuffixMatchBuilder,
    pub(crate) child_match: ChildMatchBuilder,
    pub(crate) regex_match: RegexMatchBuilder,
    pub(crate) default_next: NodeName,
}

impl RouteUpstreamEscaperConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        RouteUpstreamEscaperConfig {
            name: NodeName::default(),
            position,
            exact_match: ExactMatchBuilder::default(),
            subnet_match: SubnetMatchBuilder::default(),
            suffix_match: SuffixMatchBuilder::default(),
            child_match: ChildMatchBuilder::default(),
            regex_match: RegexMatchBuilder::default(),
            default_next: NodeName::default(),
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
            "exact_match" | "exact_rules" => self
                .exact_match
                .set_by_yaml(v)
                .context(format!("invalid exact match rules for key {k}")),
            "subnet_match" | "subnet_rules" => self
                .subnet_match
                .set_by_yaml(v)
                .context(format!("invalid subnet match rules for key {k}")),
            "suffix_match" | "suffix_rules" | "radix_match" | "radix_rules" => self
                .suffix_match
                .set_by_yaml(v)
                .context(format!("invalid suffix match rules for key {k}")),
            "child_match" | "child_rules" => self
                .child_match
                .set_by_yaml(v)
                .context(format!("invalid child match rules for key {k}")),
            "regex_match" | "regex_rules" => self
                .regex_match
                .set_by_yaml(v)
                .context(format!("invalid regex match rules for key {k}")),
            "default_next" => {
                self.default_next = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.default_next.is_empty() {
            return Err(anyhow!("no default next escaper is set"));
        }
        self.exact_match.check()?;
        self.subnet_match.check()?;
        self.suffix_match.check()?;
        self.child_match.check()?;
        self.regex_match.check()?;
        Ok(())
    }
}

impl EscaperConfig for RouteUpstreamEscaperConfig {
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
        let AnyEscaperConfig::RouteUpstream(new) = new else {
            return EscaperConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            return EscaperConfigDiffAction::NoAction;
        }

        EscaperConfigDiffAction::Reload
    }

    fn dependent_escaper(&self) -> Option<BTreeSet<NodeName>> {
        let mut set = BTreeSet::new();
        set.insert(self.default_next.clone());
        self.exact_match.collect_escaper(&mut set);
        self.subnet_match.collect_escaper(&mut set);
        self.suffix_match.collect_escaper(&mut set);
        self.child_match.collect_escaper(&mut set);
        self.regex_match.collect_escaper(&mut set);
        Some(set)
    }
}
