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

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;

use ahash::AHashMap;
use anyhow::{Context, anyhow};
use rustc_hash::FxHashMap;
use yaml_rust::Yaml;

use g3_types::metrics::NodeName;
use g3_types::net::Host;

use crate::config::escaper::verify::EscaperConfigVerifier;

#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct ExactMatchBuilder {
    domain: BTreeMap<NodeName, BTreeSet<Arc<str>>>,
    ipaddr: BTreeMap<NodeName, BTreeSet<IpAddr>>,
}

impl ExactMatchBuilder {
    pub(super) fn check(&self) -> anyhow::Result<()> {
        EscaperConfigVerifier::check_duplicated_rule(&self.domain)
            .context("found duplicated domain for exact match")?;
        EscaperConfigVerifier::check_duplicated_rule(&self.ipaddr)
            .context("found duplicated ipaddr for exact match")?;
        Ok(())
    }

    pub(super) fn collect_escaper(&self, set: &mut BTreeSet<NodeName>) {
        set.extend(self.domain.keys().cloned());
        set.extend(self.ipaddr.keys().cloned());
    }

    pub(super) fn set_by_yaml(&mut self, value: &Yaml) -> anyhow::Result<()> {
        match value {
            Yaml::Hash(map) => g3_yaml::foreach_kv(map, |k, v| {
                let escaper = NodeName::from_str(k)
                    .map_err(|e| anyhow!("the map key is not valid escaper name: {e}"))?;
                let mut match_values = ExactMatchValues::default();
                match_values
                    .set_by_yaml(v)
                    .context(format!("invalid exact match values for key {k}"))?;

                self.add_rule(escaper, match_values);
                Ok(())
            }),
            Yaml::Array(seq) => {
                for (i, v) in seq.iter().enumerate() {
                    let Yaml::Hash(map) = v else {
                        return Err(anyhow!("yaml value type for #{i} should be map"));
                    };

                    let mut escaper = NodeName::default();
                    let mut match_values = ExactMatchValues::default();
                    g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                        "next" | "escaper" => {
                            escaper = g3_yaml::value::as_metric_node_name(v)?;
                            Ok(())
                        }
                        "hosts" | "host" => match_values
                            .set_by_yaml(v)
                            .context("invalid exact match values"),
                        _ => Err(anyhow!("invalid key {k}")),
                    })
                    .context(format!("invalid exact match rule for #{i}"))?;

                    self.add_rule(escaper, match_values);
                }
                Ok(())
            }
            _ => Err(anyhow!("exact match rules should be a map or an array")),
        }
    }

    fn add_rule(&mut self, escaper: NodeName, values: ExactMatchValues) {
        let ExactMatchValues { domain, ipaddr } = values;
        if !domain.is_empty() {
            self.domain
                .entry(escaper.clone())
                .or_default()
                .extend(domain);
        }
        if !ipaddr.is_empty() {
            self.ipaddr.entry(escaper).or_default().extend(ipaddr);
        }
    }

    pub(crate) fn build<T: Clone>(&self, value_table: &BTreeMap<NodeName, T>) -> ExactMatch<T> {
        let mut exact_match_ipaddr = FxHashMap::default();
        for (escaper, ips) in &self.ipaddr {
            let Some(value) = value_table.get(escaper) else {
                continue;
            };
            for ip in ips {
                exact_match_ipaddr.insert(*ip, value.clone());
            }
        }
        let mut exact_match_domain = AHashMap::new();
        for (escaper, hosts) in &self.domain {
            for host in hosts {
                let Some(value) = value_table.get(escaper) else {
                    continue;
                };
                exact_match_domain.insert(host.clone(), value.clone());
            }
        }
        ExactMatch {
            ipaddr: exact_match_ipaddr,
            domain: exact_match_domain,
        }
    }
}

#[derive(Default)]
struct ExactMatchValues {
    domain: BTreeSet<Arc<str>>,
    ipaddr: BTreeSet<IpAddr>,
}

impl ExactMatchValues {
    fn set_by_yaml(&mut self, value: &Yaml) -> anyhow::Result<()> {
        if let Yaml::Array(seq) = value {
            for (i, v) in seq.iter().enumerate() {
                self.add_yaml_value(v)
                    .context(format!("invalid exact match value for #{i}"))?;
            }
        } else {
            self.add_yaml_value(value)?;
        }
        Ok(())
    }

    fn add_yaml_value(&mut self, value: &Yaml) -> anyhow::Result<()> {
        let host = g3_yaml::value::as_host(value)?;
        match host {
            Host::Domain(domain) => {
                self.domain.insert(domain);
            }
            Host::Ip(ip) => {
                self.ipaddr.insert(ip);
            }
        }
        Ok(())
    }
}

pub(crate) struct ExactMatch<T> {
    ipaddr: FxHashMap<IpAddr, T>,
    domain: AHashMap<Arc<str>, T>,
}

impl<T> ExactMatch<T> {
    pub(crate) fn check_domain(&self, domain: &str) -> Option<&T> {
        self.domain.get(domain)
    }

    pub(crate) fn check_ip(&self, ip: IpAddr) -> Option<&T> {
        self.ipaddr.get(&ip)
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
          host: abc.example.net
        - next: escaper_2
          hosts:
            - example.com
            - 192.168.1.1
        "#;

        let v = YamlLoader::load_from_str(conf).unwrap();
        let mut builder = ExactMatchBuilder::default();
        builder.set_by_yaml(&v[0]).unwrap();

        let mut value_map = BTreeMap::new();
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_1") }, "escaper_1");
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_2") }, "escaper_2");
        let exact_match = builder.build(&value_map);

        let value = *exact_match.check_domain("abc.example.net").unwrap();
        assert!(value.eq("escaper_1"));
        assert!(exact_match.check_domain("abcexample.net").is_none());
        let value = *exact_match.check_domain("example.com").unwrap();
        assert!(value.eq("escaper_2"));
        let value = *exact_match
            .check_ip(IpAddr::from_str("192.168.1.1").unwrap())
            .unwrap();
        assert!(value.eq("escaper_2"));
        assert!(
            exact_match
                .check_ip(IpAddr::from_str("192.168.1.2").unwrap())
                .is_none()
        );
    }

    #[test]
    fn yaml_map() {
        let conf = r#"
        escaper_1:
          - abc.example.net
        escaper_2:
          - example.com
          - 192.168.1.1
        "#;

        let v = YamlLoader::load_from_str(conf).unwrap();
        let mut builder = ExactMatchBuilder::default();
        builder.set_by_yaml(&v[0]).unwrap();

        let mut value_map = BTreeMap::new();
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_1") }, "escaper_1");
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_2") }, "escaper_2");
        let exact_match = builder.build(&value_map);

        let value = *exact_match.check_domain("abc.example.net").unwrap();
        assert!(value.eq("escaper_1"));
        assert!(exact_match.check_domain("abcexample.net").is_none());
        let value = *exact_match.check_domain("example.com").unwrap();
        assert!(value.eq("escaper_2"));
        let value = *exact_match
            .check_ip(IpAddr::from_str("192.168.1.1").unwrap())
            .unwrap();
        assert!(value.eq("escaper_2"));
        assert!(
            exact_match
                .check_ip(IpAddr::from_str("192.168.1.2").unwrap())
                .is_none()
        );
    }
}
