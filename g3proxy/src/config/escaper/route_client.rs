/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;

use anyhow::{Context, anyhow};
use g3_types::metrics::NodeName;
use ip_network::IpNetwork;
use yaml_rust::{Yaml, yaml};

use g3_yaml::YamlDocPosition;

use super::{AnyEscaperConfig, EscaperConfig, EscaperConfigDiffAction, EscaperConfigVerifier};

const ESCAPER_CONFIG_TYPE: &str = "RouteClient";

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct RouteClientEscaperConfig {
    pub(crate) name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) exact_match_ipaddr: BTreeMap<NodeName, BTreeSet<IpAddr>>,
    pub(crate) subnet_match_ipaddr: BTreeMap<NodeName, BTreeSet<IpNetwork>>,
    pub(crate) default_next: NodeName,
}

impl RouteClientEscaperConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        RouteClientEscaperConfig {
            name: NodeName::default(),
            position,
            exact_match_ipaddr: BTreeMap::new(),
            subnet_match_ipaddr: BTreeMap::new(),
            default_next: NodeName::default(),
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
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "exact_match" | "exact_rules" => {
                RouteClientEscaperConfig::foreach_rule(k, v, |map| self.add_exact_match(map))
            }
            "subnet_match" | "subnet_rules" => {
                RouteClientEscaperConfig::foreach_rule(k, v, |map| self.add_subnet_match(map))
            }
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
        if !self.exact_match_ipaddr.is_empty() {
            EscaperConfigVerifier::check_duplicated_rule(&self.exact_match_ipaddr)
                .context("found duplicated ipaddr for exact match")?;
        }
        if !self.subnet_match_ipaddr.is_empty() {
            EscaperConfigVerifier::check_duplicated_rule(&self.subnet_match_ipaddr)
                .context("found duplicated subnet for subnet match")?;
        }
        Ok(())
    }

    fn add_exact_match(&mut self, map: &yaml::Hash) -> anyhow::Result<()> {
        let mut escaper = NodeName::default();
        let mut all_ipaddr = BTreeSet::<IpAddr>::new();
        g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
            "next" | "escaper" => {
                escaper = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "ips" | "ip" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let ip = g3_yaml::value::as_ipaddr(v)
                            .context(format!("invalid host value for {k}:{i}"))?;
                        all_ipaddr.insert(ip);
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
        Ok(())
    }

    fn add_subnet_match(&mut self, map: &yaml::Hash) -> anyhow::Result<()> {
        let mut escaper = NodeName::default();
        let mut all_subnets = BTreeSet::<IpNetwork>::new();
        g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
            "next" | "escaper" => {
                escaper = g3_yaml::value::as_metric_node_name(v)?;
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
}

impl EscaperConfig for RouteClientEscaperConfig {
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
        let AnyEscaperConfig::RouteClient(new) = new else {
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
        for key in self.exact_match_ipaddr.keys() {
            set.insert(key.clone());
        }
        for key in self.subnet_match_ipaddr.keys() {
            set.insert(key.clone());
        }
        Some(set)
    }
}
