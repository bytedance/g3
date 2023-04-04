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
use std::net::IpAddr;

use anyhow::{anyhow, Context};
use ip_network::IpNetwork;
use yaml_rust::{yaml, Yaml};

use g3_types::metrics::MetricsName;
use g3_types::net::Host;
use g3_yaml::YamlDocPosition;

use super::{AnyEscaperConfig, EscaperConfig, EscaperConfigDiffAction, EscaperConfigVerifier};

const ESCAPER_CONFIG_TYPE: &str = "RouteUpstream";

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct RouteUpstreamEscaperConfig {
    pub(crate) name: MetricsName,
    position: Option<YamlDocPosition>,
    pub(crate) exact_match_domain: BTreeMap<MetricsName, BTreeSet<String>>,
    pub(crate) exact_match_ipaddr: BTreeMap<MetricsName, BTreeSet<IpAddr>>,
    pub(crate) subnet_match_ipaddr: BTreeMap<MetricsName, BTreeSet<IpNetwork>>,
    pub(crate) radix_match_domain: BTreeMap<MetricsName, BTreeSet<String>>,
    pub(crate) child_match_domain: BTreeMap<MetricsName, BTreeSet<String>>,
    pub(crate) default_next: MetricsName,
}

impl RouteUpstreamEscaperConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        RouteUpstreamEscaperConfig {
            name: MetricsName::default(),
            position,
            exact_match_domain: BTreeMap::new(),
            exact_match_ipaddr: BTreeMap::new(),
            subnet_match_ipaddr: BTreeMap::new(),
            radix_match_domain: BTreeMap::new(),
            child_match_domain: BTreeMap::new(),
            default_next: MetricsName::default(),
        }
    }

    fn foreach_rule<F>(k: &str, v: &Yaml, mut f: F) -> anyhow::Result<()>
    where
        F: FnMut(&yaml::Hash) -> anyhow::Result<()>,
    {
        if let Yaml::Array(seq) = v {
            for (i, rule) in seq.iter().enumerate() {
                if let Yaml::Hash(map) = rule {
                    f(map).context(format!("failed to parse rule {k}#{i}"))?;
                } else {
                    return Err(anyhow!("invalid value type for {k}#{i}"));
                }
            }
            Ok(())
        } else {
            Err(anyhow!("invalid array value for key {k}"))
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
            "exact_match" | "exact_rules" => {
                RouteUpstreamEscaperConfig::foreach_rule(k, v, |map| self.add_exact_match(map))
            }
            "subnet_match" | "subnet_rules" => {
                RouteUpstreamEscaperConfig::foreach_rule(k, v, |map| self.add_subnet_match(map))
            }
            "radix_match" | "radix_rules" => {
                RouteUpstreamEscaperConfig::foreach_rule(k, v, |map| self.add_radix_match(map))
            }
            "child_match" | "child_rules" => {
                RouteUpstreamEscaperConfig::foreach_rule(k, v, |map| self.add_child_match(map))
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
        if self.default_next.is_empty() {
            return Err(anyhow!("no default next escaper is set"));
        }
        if !self.exact_match_ipaddr.is_empty() {
            EscaperConfigVerifier::check_duplicated_rule(&self.exact_match_ipaddr)
                .context("found duplicated ipaddr for exact match")?;
        }
        if !self.subnet_match_ipaddr.is_empty() {
            EscaperConfigVerifier::check_duplicated_rule(&self.subnet_match_ipaddr)
                .context("found duplicated subnet for subnet match")?;
        }
        if !self.exact_match_domain.is_empty() {
            EscaperConfigVerifier::check_duplicated_rule(&self.exact_match_domain)
                .context("found duplicated domain for exact match")?;
        }
        if !self.radix_match_domain.is_empty() {
            EscaperConfigVerifier::check_duplicated_rule(&self.radix_match_domain)
                .context("found duplicated domain suffix for radix match")?;
        }
        if !self.child_match_domain.is_empty() {
            EscaperConfigVerifier::check_duplicated_rule(&self.child_match_domain)
                .context("found duplicated domain suffix for child match")?;
        }
        Ok(())
    }

    fn add_exact_match(&mut self, map: &yaml::Hash) -> anyhow::Result<()> {
        let mut escaper = MetricsName::default();
        let mut all_ipaddr = BTreeSet::<IpAddr>::new();
        let mut all_domain = BTreeSet::<String>::new();
        g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
            "next" | "escaper" => {
                escaper = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "hosts" | "host" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        match g3_yaml::value::as_host(v)
                            .context(format!("invalid host value for {k}:{i}"))?
                        {
                            Host::Ip(ip) => {
                                all_ipaddr.insert(ip);
                            }
                            Host::Domain(domain) => {
                                all_domain.insert(domain);
                            }
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
        if !all_ipaddr.is_empty() {
            if let Some(_old) = self.exact_match_ipaddr.insert(escaper.clone(), all_ipaddr) {
                return Err(anyhow!("found multiple entries for next escaper {escaper}"));
            }
        }
        if !all_domain.is_empty() {
            if let Some(_old) = self.exact_match_domain.insert(escaper.clone(), all_domain) {
                return Err(anyhow!("found multiple entries for next escaper {escaper}"));
            }
        }
        Ok(())
    }

    fn add_subnet_match(&mut self, map: &yaml::Hash) -> anyhow::Result<()> {
        let mut escaper = MetricsName::default();
        let mut all_subnets = BTreeSet::<IpNetwork>::new();
        g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
            "next" | "escaper" => {
                escaper = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "subnets" | "subnet" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let subnet = g3_yaml::value::as_ip_network(v)
                            .context(format!("invalid subnet value for {k}:{i}"))?;
                        all_subnets.insert(subnet);
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
        if !all_subnets.is_empty() {
            if let Some(_old) = self
                .subnet_match_ipaddr
                .insert(escaper.clone(), all_subnets)
            {
                return Err(anyhow!("found multiple entries for next escaper {escaper}"));
            }
        }
        Ok(())
    }

    fn add_child_match(&mut self, map: &yaml::Hash) -> anyhow::Result<()> {
        let mut escaper = MetricsName::default();
        let mut all_domain = BTreeSet::<String>::new();
        g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
            "next" | "escaper" => {
                escaper = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "domains" | "domain" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let domain = g3_yaml::value::as_domain(v)
                            .context(format!("invalid domain suffix value for {k}:{i}"))?;
                        all_domain.insert(domain);
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
        if !all_domain.is_empty() {
            if let Some(_old) = self.child_match_domain.insert(escaper.clone(), all_domain) {
                return Err(anyhow!("found multiple entries for next escaper {escaper}"));
            }
        }
        Ok(())
    }

    fn add_radix_match(&mut self, map: &yaml::Hash) -> anyhow::Result<()> {
        let mut escaper = MetricsName::default();
        let mut all_domain = BTreeSet::<String>::new();
        g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
            "next" | "escaper" => {
                escaper = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "suffixes" | "suffix" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let domain = g3_yaml::value::as_domain(v)
                            .context(format!("invalid domain suffix value for {k}:{i}"))?;
                        all_domain.insert(domain);
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
        if !all_domain.is_empty() {
            if let Some(_old) = self.radix_match_domain.insert(escaper.clone(), all_domain) {
                return Err(anyhow!("found multiple entries for next escaper {escaper}"));
            }
        }
        Ok(())
    }
}

impl EscaperConfig for RouteUpstreamEscaperConfig {
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
            AnyEscaperConfig::RouteUpstream(config) => config,
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
        for key in self.exact_match_ipaddr.keys() {
            set.insert(key.clone());
        }
        for key in self.exact_match_domain.keys() {
            set.insert(key.clone());
        }
        for key in self.subnet_match_ipaddr.keys() {
            set.insert(key.clone());
        }
        for key in self.radix_match_domain.keys() {
            set.insert(key.clone());
        }
        for key in self.child_match_domain.keys() {
            set.insert(key.clone());
        }
        Some(set)
    }
}
