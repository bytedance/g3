/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::str::FromStr;

use anyhow::{Context, anyhow};
use ip_network::IpNetwork;
use ip_network_table::IpNetworkTable;
use yaml_rust::Yaml;

use g3_types::metrics::NodeName;

use crate::config::escaper::verify::EscaperConfigVerifier;

#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct SubnetMatchBuilder {
    inner: BTreeMap<NodeName, BTreeSet<IpNetwork>>,
}

impl SubnetMatchBuilder {
    pub(super) fn check(&self) -> anyhow::Result<()> {
        EscaperConfigVerifier::check_duplicated_rule(&self.inner)
            .context("found duplicated rule for subnet match")?;
        Ok(())
    }

    pub(super) fn collect_escaper(&self, set: &mut BTreeSet<NodeName>) {
        set.extend(self.inner.keys().cloned())
    }

    pub(super) fn set_by_yaml(&mut self, value: &Yaml) -> anyhow::Result<()> {
        match value {
            Yaml::Hash(map) => g3_yaml::foreach_kv(map, |k, v| {
                let escaper = NodeName::from_str(k)
                    .map_err(|e| anyhow!("the map key is not valid escaper name: {e}"))?;
                let subnets = g3_yaml::value::as_list(v, g3_yaml::value::as_ip_network)?;
                self.add_rule(escaper, subnets);
                Ok(())
            }),
            Yaml::Array(seq) => {
                for (i, v) in seq.iter().enumerate() {
                    let Yaml::Hash(map) = v else {
                        return Err(anyhow!("yaml value type for #{i} should be map"));
                    };

                    let mut escaper = NodeName::default();
                    let mut subnets = Vec::new();
                    g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                        "next" | "escaper" => {
                            escaper = g3_yaml::value::as_metric_node_name(v)?;
                            Ok(())
                        }
                        "subnets" | "subnet" => {
                            subnets = g3_yaml::value::as_list(v, g3_yaml::value::as_ip_network)?;
                            Ok(())
                        }
                        _ => Err(anyhow!("invalid key {k}")),
                    })
                    .context(format!("invalid subnet match rule for #{i}"))?;

                    self.add_rule(escaper, subnets);
                }
                Ok(())
            }
            _ => Err(anyhow!("subnet match rules should be a map or an array")),
        }
    }

    fn add_rule(&mut self, escaper: NodeName, domains: Vec<IpNetwork>) {
        self.inner.entry(escaper).or_default().extend(domains);
    }

    pub(crate) fn build<T: Clone>(
        &self,
        value_table: &BTreeMap<NodeName, T>,
    ) -> Option<SubnetMatch<T>> {
        if self.inner.is_empty() {
            return None;
        }

        let mut table = IpNetworkTable::new();
        for (escaper, subnets) in &self.inner {
            for subnet in subnets {
                let Some(next) = value_table.get(escaper) else {
                    continue;
                };
                table.insert(*subnet, next.clone());
            }
        }
        Some(SubnetMatch { inner: table })
    }
}

pub(crate) struct SubnetMatch<T> {
    inner: IpNetworkTable<T>,
}

impl<T> SubnetMatch<T> {
    pub(crate) fn check_ip(&self, ip: IpAddr) -> Option<&T> {
        self.inner.longest_match(ip).map(|(_net, value)| value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    #[test]
    fn yaml_seq() {
        let conf = r#"
        - next: escaper_1
          subnet: 192.168.0.0/16
        - next: escaper_2
          subnet:
            - 192.168.0.0/24
            - fe80::/64
        "#;

        let v = YamlLoader::load_from_str(conf).unwrap();
        let mut builder = SubnetMatchBuilder::default();
        builder.set_by_yaml(&v[0]).unwrap();

        let mut value_map = BTreeMap::new();
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_1") }, "escaper_1");
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_2") }, "escaper_2");
        let exact_match = builder.build(&value_map).unwrap();

        let value = *exact_match
            .check_ip(IpAddr::from_str("192.168.1.1").unwrap())
            .unwrap();
        assert!(value.eq("escaper_1"));
        let value = *exact_match
            .check_ip(IpAddr::from_str("192.168.0.1").unwrap())
            .unwrap();
        assert!(value.eq("escaper_2"));
        assert!(
            exact_match
                .check_ip(IpAddr::from_str("172.18.0.0").unwrap())
                .is_none()
        );
        let value = *exact_match
            .check_ip(IpAddr::from_str("fe80::1").unwrap())
            .unwrap();
        assert!(value.eq("escaper_2"));
    }

    #[test]
    fn yaml_map() {
        let conf = r#"
        escaper_1:
          - 192.168.0.0/16
        escaper_2:
          - 192.168.0.0/24
          - fe80::/64
        "#;

        let v = YamlLoader::load_from_str(conf).unwrap();
        let mut builder = SubnetMatchBuilder::default();
        builder.set_by_yaml(&v[0]).unwrap();

        let mut value_map = BTreeMap::new();
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_1") }, "escaper_1");
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_2") }, "escaper_2");
        let exact_match = builder.build(&value_map).unwrap();

        let value = *exact_match
            .check_ip(IpAddr::from_str("192.168.1.1").unwrap())
            .unwrap();
        assert!(value.eq("escaper_1"));
        let value = *exact_match
            .check_ip(IpAddr::from_str("192.168.0.1").unwrap())
            .unwrap();
        assert!(value.eq("escaper_2"));
        assert!(
            exact_match
                .check_ip(IpAddr::from_str("172.18.0.0").unwrap())
                .is_none()
        );
        let value = *exact_match
            .check_ip(IpAddr::from_str("fe80::1").unwrap())
            .unwrap();
        assert!(value.eq("escaper_2"));
    }
}
