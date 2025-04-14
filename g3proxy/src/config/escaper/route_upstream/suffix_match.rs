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
use std::str::FromStr;

use anyhow::{Context, anyhow};
use radix_trie::{Trie, TrieCommon};
use yaml_rust::Yaml;

use g3_types::metrics::NodeName;

use crate::config::escaper::verify::EscaperConfigVerifier;

#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct SuffixMatchBuilder {
    inner: BTreeMap<NodeName, BTreeSet<String>>,
}

impl SuffixMatchBuilder {
    pub(super) fn check(&self) -> anyhow::Result<()> {
        EscaperConfigVerifier::check_duplicated_rule(&self.inner)
            .context("found duplicated rule for suffix match")?;
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
                let suffixes = g3_yaml::value::as_list(v, g3_yaml::value::as_domain)?;
                self.add_rule(escaper, suffixes);
                Ok(())
            }),
            Yaml::Array(seq) => {
                for (i, v) in seq.iter().enumerate() {
                    let Yaml::Hash(map) = v else {
                        return Err(anyhow!("yaml value type for #{i} should be map"));
                    };

                    let mut escaper = NodeName::default();
                    let mut suffixes = Vec::new();
                    g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                        "next" | "escaper" => {
                            escaper = g3_yaml::value::as_metric_node_name(v)?;
                            Ok(())
                        }
                        "suffixes" | "suffix" => {
                            suffixes = g3_yaml::value::as_list(v, g3_yaml::value::as_domain)?;
                            Ok(())
                        }
                        _ => Err(anyhow!("invalid key {k}")),
                    })
                    .context(format!("invalid suffix match rule for #{i}"))?;

                    self.add_rule(escaper, suffixes);
                }
                Ok(())
            }
            _ => Err(anyhow!("suffix match rules should be a map or an array")),
        }
    }

    fn add_rule(&mut self, escaper: NodeName, domains: Vec<String>) {
        self.inner.entry(escaper).or_default().extend(domains);
    }

    pub(crate) fn build<T: Clone>(
        &self,
        value_table: &BTreeMap<NodeName, T>,
    ) -> Option<SuffixMatch<T>> {
        if self.inner.is_empty() {
            return None;
        }

        let mut trie = Trie::new();
        for (escaper, domains) in &self.inner {
            for domain in domains {
                let Some(value) = value_table.get(escaper) else {
                    continue;
                };
                let reversed = domain.chars().rev().collect();
                trie.insert(reversed, value.clone());
            }
        }
        if trie.is_empty() {
            None
        } else {
            Some(SuffixMatch { inner: trie })
        }
    }
}

pub(crate) struct SuffixMatch<T> {
    inner: Trie<String, T>,
}

impl<T> SuffixMatch<T> {
    pub(crate) fn check_domain(&self, domain: &str) -> Option<&T> {
        let key: String = domain.chars().rev().collect();
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
          suffix: example.net
        - next: escaper_2
          suffix:
            - a.example.net
            - cd.example.org
        "#;

        let v = YamlLoader::load_from_str(conf).unwrap();
        let mut builder = SuffixMatchBuilder::default();
        builder.set_by_yaml(&v[0]).unwrap();

        let mut value_map = BTreeMap::new();
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_1") }, "escaper_1");
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_2") }, "escaper_2");
        let suffix_match = builder.build(&value_map).unwrap();

        let value = *suffix_match.check_domain("abc.example.net").unwrap();
        assert!(value.eq("escaper_1"));
        let value = *suffix_match.check_domain("abcexample.net").unwrap();
        assert!(value.eq("escaper_1"));
        let value = *suffix_match.check_domain("ba.example.net").unwrap();
        assert!(value.eq("escaper_2"));
        assert!(suffix_match.check_domain("cde.example.org").is_none());
        let value = *suffix_match.check_domain("a.cd.example.org").unwrap();
        assert!(value.eq("escaper_2"));
    }

    #[test]
    fn yaml_map() {
        let conf = r#"
        escaper_1:
          - example.net
        escaper_2:
          - a.example.net
          - cd.example.org
        "#;

        let v = YamlLoader::load_from_str(conf).unwrap();
        let mut builder = SuffixMatchBuilder::default();
        builder.set_by_yaml(&v[0]).unwrap();

        let mut value_map = BTreeMap::new();
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_1") }, "escaper_1");
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_2") }, "escaper_2");
        let suffix_match = builder.build(&value_map).unwrap();

        let value = *suffix_match.check_domain("abc.example.net").unwrap();
        assert!(value.eq("escaper_1"));
        let value = *suffix_match.check_domain("abcexample.net").unwrap();
        assert!(value.eq("escaper_1"));
        let value = *suffix_match.check_domain("ba.example.net").unwrap();
        assert!(value.eq("escaper_2"));
        assert!(suffix_match.check_domain("cde.example.org").is_none());
        let value = *suffix_match.check_domain("a.cd.example.org").unwrap();
        assert!(value.eq("escaper_2"));
    }
}
