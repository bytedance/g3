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

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;
use std::time::Duration;

use anyhow::{anyhow, Context};
use ip_network::IpNetwork;
use yaml_rust::{yaml, Yaml};

use g3_types::metrics::MetricsName;
use g3_types::resolve::ResolveStrategy;
use g3_yaml::YamlDocPosition;

use super::{AnyEscaperConfig, EscaperConfig, EscaperConfigDiffAction, EscaperConfigVerifier};

const ESCAPER_CONFIG_TYPE: &str = "RouteResolved";

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct RouteResolvedEscaperConfig {
    pub(crate) name: MetricsName,
    position: Option<YamlDocPosition>,
    pub(crate) resolver: MetricsName,
    pub(crate) resolve_strategy: ResolveStrategy,
    pub(crate) resolution_delay: Duration,
    pub(crate) lpm_rules: BTreeMap<MetricsName, BTreeSet<IpNetwork>>,
    pub(crate) default_next: MetricsName,
}

impl RouteResolvedEscaperConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        RouteResolvedEscaperConfig {
            name: MetricsName::default(),
            position,
            resolver: MetricsName::default(),
            resolve_strategy: Default::default(),
            resolution_delay: Duration::from_millis(50),
            lpm_rules: BTreeMap::new(),
            default_next: MetricsName::default(),
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
            "resolver" => {
                self.resolver = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "resolve_strategy" => {
                self.resolve_strategy = g3_yaml::value::as_resolve_strategy(v)?;
                Ok(())
            }
            "resolution_delay" => {
                self.resolution_delay = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "lpm_match" | "lpm_rules" => {
                if let Yaml::Array(seq) = v {
                    for (i, rule) in seq.iter().enumerate() {
                        if let Yaml::Hash(map) = rule {
                            self.add_lpm_rule(map)?;
                        } else {
                            return Err(anyhow!("invalid value type for {k}#{i}"));
                        }
                    }
                    Ok(())
                } else {
                    Err(anyhow!("invalid array value for key {k}"))
                }
            }
            "default_next" => {
                self.default_next = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.resolver.is_empty() {
            return Err(anyhow!("no resolver is set"));
        }
        if self.default_next.is_empty() {
            return Err(anyhow!("no default next escaper is set"));
        }
        if !self.lpm_rules.is_empty() {
            EscaperConfigVerifier::check_duplicated_rule(&self.lpm_rules)
                .context("found duplicated network")?;
        }
        Ok(())
    }

    fn add_lpm_rule(&mut self, map: &yaml::Hash) -> anyhow::Result<()> {
        let mut escaper = MetricsName::default();
        let mut networks = BTreeSet::<IpNetwork>::new();
        g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
            "next" | "escaper" => {
                escaper = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "nets" | "net" | "networks" | "network" => {
                if let Yaml::Array(seq) = v {
                    for (i, obj) in seq.iter().enumerate() {
                        if let Yaml::String(net) = obj {
                            let net = IpNetwork::from_str(net).map_err(|e| {
                                anyhow!("invalid network string for {k}#{i}: {e:?}")
                            })?;
                            networks.insert(net);
                        } else {
                            return Err(anyhow!("invalid network string value for {k}#{i}"));
                        }
                    }
                    Ok(())
                } else {
                    Err(anyhow!("invalid array value for key {k}"))
                }
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;
        if escaper.is_empty() {
            return Err(anyhow!("no next escaper set"));
        }
        if let Some(_old) = self.lpm_rules.insert(escaper.clone(), networks) {
            return Err(anyhow!("found multiple entries for next escaper {escaper}"));
        }
        Ok(())
    }
}

impl EscaperConfig for RouteResolvedEscaperConfig {
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
        &self.resolver
    }

    fn diff_action(&self, new: &AnyEscaperConfig) -> EscaperConfigDiffAction {
        let new = match new {
            AnyEscaperConfig::RouteResolved(config) => config,
            _ => return EscaperConfigDiffAction::SpawnNew,
        };

        if self.eq(new) {
            return EscaperConfigDiffAction::NoAction;
        }

        EscaperConfigDiffAction::Reload
    }

    fn dependent_escaper(&self) -> Option<BTreeSet<MetricsName>> {
        let mut set = BTreeSet::new();
        set.insert(self.default_next.clone());
        for key in self.lpm_rules.keys() {
            set.insert(key.clone());
        }
        Some(set)
    }
}
