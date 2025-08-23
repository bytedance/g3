/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use anyhow::{Context, anyhow};
use radix_trie::{Trie, TrieCommon};
use yaml_rust::Yaml;

use g3_types::metrics::NodeName;

use crate::config::escaper::verify::EscaperConfigVerifier;

#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct ChildMatchBuilder {
    inner: BTreeMap<NodeName, BTreeSet<String>>,
}

impl ChildMatchBuilder {
    pub(super) fn check(&self) -> anyhow::Result<()> {
        EscaperConfigVerifier::check_duplicated_rule(&self.inner)
            .context("found duplicated rule for child match")?;
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
                let domains = g3_yaml::value::as_list(v, g3_yaml::value::as_domain)?;
                self.add_rule(escaper, domains);
                Ok(())
            }),
            Yaml::Array(seq) => {
                for (i, v) in seq.iter().enumerate() {
                    let Yaml::Hash(map) = v else {
                        return Err(anyhow!("yaml value type for #{i} should be map"));
                    };

                    let mut escaper = NodeName::default();
                    let mut domains = Vec::new();
                    g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                        "next" | "escaper" => {
                            escaper = g3_yaml::value::as_metric_node_name(v)?;
                            Ok(())
                        }
                        "domains" | "domain" => {
                            domains = g3_yaml::value::as_list(v, g3_yaml::value::as_domain)?;
                            Ok(())
                        }
                        _ => Err(anyhow!("invalid key {k}")),
                    })
                    .context(format!("invalid child match rule for #{i}"))?;

                    self.add_rule(escaper, domains);
                }
                Ok(())
            }
            _ => Err(anyhow!("child match rules should be a map or an array")),
        }
    }

    fn add_rule(&mut self, escaper: NodeName, domains: Vec<String>) {
        self.inner.entry(escaper).or_default().extend(domains);
    }

    pub(crate) fn build<T: Clone>(
        &self,
        value_table: &BTreeMap<NodeName, T>,
    ) -> Option<ChildMatch<T>> {
        if self.inner.is_empty() {
            return None;
        }

        let mut trie = Trie::new();
        for (escaper, domains) in &self.inner {
            for domain in domains {
                let Some(value) = value_table.get(escaper) else {
                    continue;
                };
                let reversed = g3_types::resolve::reverse_idna_domain(domain);
                trie.insert(reversed, value.clone());
            }
        }
        if trie.is_empty() {
            None
        } else {
            Some(ChildMatch { inner: trie })
        }
    }
}

pub(crate) struct ChildMatch<T> {
    inner: Trie<String, T>,
}

impl<T> ChildMatch<T> {
    pub(crate) fn check_domain(&self, domain: &str) -> Option<&T> {
        let key = g3_types::resolve::reverse_idna_domain(domain);
        self.inner.get_ancestor_value(&key)
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
          domain: example.net
        - next: escaper_2
          domains:
            - example.com
            - example.org
        "#;

        let v = YamlLoader::load_from_str(conf).unwrap();
        let mut builder = ChildMatchBuilder::default();
        builder.set_by_yaml(&v[0]).unwrap();

        let mut value_map = BTreeMap::new();
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_1") }, "escaper_1");
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_2") }, "escaper_2");
        let child_match = builder.build(&value_map).unwrap();

        let value = *child_match.check_domain("abc.example.net").unwrap();
        assert!(value.eq("escaper_1"));
        assert!(child_match.check_domain("abcexample.net").is_none());
        let value = *child_match.check_domain("cde1.example.com").unwrap();
        assert!(value.eq("escaper_2"));
        assert!(child_match.check_domain("cde.example.info").is_none());
    }

    #[test]
    fn yaml_map() {
        let conf = r#"
        escaper_1:
          - example.net
        escaper_2:
          - example.com
          - example.org
        "#;

        let v = YamlLoader::load_from_str(conf).unwrap();
        let mut builder = ChildMatchBuilder::default();
        builder.set_by_yaml(&v[0]).unwrap();

        let mut value_map = BTreeMap::new();
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_1") }, "escaper_1");
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_2") }, "escaper_2");
        let child_match = builder.build(&value_map).unwrap();

        let value = *child_match.check_domain("abc.example.net").unwrap();
        assert!(value.eq("escaper_1"));
        assert!(child_match.check_domain("abcexample.net").is_none());
        let value = *child_match.check_domain("cde1.example.com").unwrap();
        assert!(value.eq("escaper_2"));
        assert!(child_match.check_domain("cde.example.info").is_none());
    }
}
