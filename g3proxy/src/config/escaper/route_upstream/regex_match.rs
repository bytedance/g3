/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use anyhow::{Context, anyhow};
use radix_trie::{Trie, TrieCommon};
use regex::RegexSet;
use yaml_rust::Yaml;

use crate::config::escaper::verify::EscaperConfigVerifier;

use g3_types::metrics::NodeName;

#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct RegexMatchBuilder {
    inner: BTreeMap<NodeName, BTreeSet<RegexMatchValue>>,
}

impl RegexMatchBuilder {
    pub(super) fn check(&self) -> anyhow::Result<()> {
        EscaperConfigVerifier::check_duplicated_rule(&self.inner)
            .context("found duplicated rule for regex match")?;
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
                let regexes = g3_yaml::value::as_list(v, RegexMatchValue::parse_yaml)
                    .context(format!("invalid regex match rule values for key {k}"))?;
                self.add_rule(escaper, regexes);
                Ok(())
            }),
            Yaml::Array(seq) => {
                for (i, v) in seq.iter().enumerate() {
                    let Yaml::Hash(map) = v else {
                        return Err(anyhow!("yaml value type for #{i} should be map"));
                    };

                    let mut escaper = NodeName::default();
                    let mut regexes = Vec::new();
                    g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                        "next" | "escaper" => {
                            escaper = g3_yaml::value::as_metric_node_name(v)?;
                            Ok(())
                        }
                        "rules" | "rule" => {
                            regexes = g3_yaml::value::as_list(v, RegexMatchValue::parse_yaml)?;
                            Ok(())
                        }
                        _ => Err(anyhow!("invalid key {k}")),
                    })
                    .context(format!("invalid child match rule for #{i}"))?;

                    self.add_rule(escaper, regexes);
                }
                Ok(())
            }
            _ => Err(anyhow!("child match rules should be a map or an array")),
        }
    }

    fn add_rule(&mut self, escaper: NodeName, regexes: Vec<RegexMatchValue>) {
        self.inner.entry(escaper).or_default().extend(regexes);
    }

    pub(crate) fn build<T: Clone>(
        &self,
        value_table: &BTreeMap<NodeName, T>,
    ) -> Option<RegexMatch<T>> {
        if self.inner.is_empty() {
            return None;
        }

        let mut parent_match_map: BTreeMap<String, Vec<(RegexSet, T)>> = BTreeMap::new();
        let mut full_match_vec = Vec::new();
        for (escaper, rules) in &self.inner {
            let mut parent_regex_map: BTreeMap<String, Vec<&str>> = BTreeMap::new();
            let mut full_regex_set: BTreeSet<&str> = BTreeSet::new();
            for rule in rules {
                if rule.parent_domain.is_empty() {
                    full_regex_set.insert(&rule.sub_domain_regex);
                } else {
                    let parent_reversed =
                        g3_types::resolve::reverse_idna_domain(&rule.parent_domain);
                    parent_regex_map
                        .entry(parent_reversed)
                        .or_default()
                        .push(&rule.sub_domain_regex);
                }
            }

            let Some(value) = value_table.get(escaper) else {
                unreachable!()
            };
            for (parent_domain, regexes) in parent_regex_map {
                let Ok(regex_set) = RegexSet::new(regexes) else {
                    unreachable!()
                };
                parent_match_map
                    .entry(parent_domain)
                    .or_default()
                    .push((regex_set, value.clone()));
            }
            if !full_regex_set.is_empty() {
                let regex_set = RegexSet::new(full_regex_set).unwrap();
                full_match_vec.push((regex_set, value.clone()));
            }
        }
        let mut parent_match_trie = Trie::new();
        for (parent_domain, value) in parent_match_map {
            parent_match_trie.insert(parent_domain, value);
        }
        if parent_match_trie.is_empty() {
            None
        } else {
            Some(RegexMatch {
                parent_match: parent_match_trie,
                full_match: full_match_vec,
            })
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RegexMatchValue {
    parent_domain: String,
    sub_domain_regex: String,
}

impl fmt::Display for RegexMatchValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "regex {} for parent domain {}",
            self.sub_domain_regex, self.parent_domain
        )
    }
}

impl RegexMatchValue {
    fn parse_yaml(value: &Yaml) -> anyhow::Result<Self> {
        let mut match_value = RegexMatchValue::default();
        match value {
            Yaml::Hash(map) => {
                g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                    "parent" => {
                        match_value.parent_domain = g3_yaml::value::as_domain(v)?;
                        Ok(())
                    }
                    "regex" => {
                        let regex = g3_yaml::value::as_regex(v)?;
                        match_value.sub_domain_regex = regex.to_string();
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid key {k}")),
                })?
            }
            Yaml::String(_) => {
                let regex = g3_yaml::value::as_regex(value)?;
                match_value.sub_domain_regex = regex.to_string();
            }
            _ => {
                return Err(anyhow!("invalid value type for regex match rule value"));
            }
        }
        if match_value.sub_domain_regex.is_empty() {
            return Err(anyhow!("no regular expression set"));
        }
        Ok(match_value)
    }
}

pub(crate) struct RegexMatch<T> {
    parent_match: Trie<String, Vec<(RegexSet, T)>>,
    full_match: Vec<(RegexSet, T)>,
}

impl<T> RegexMatch<T> {
    pub(crate) fn check_domain(&self, domain: &str) -> Option<&T> {
        let key: String = g3_types::resolve::reverse_idna_domain(domain);
        if let Some(sub_trie) = self.parent_match.get_ancestor(&key) {
            if let Some(rules) = sub_trie.value() {
                let suffix_len = sub_trie.prefix().as_bytes().len();
                let prefix = if domain.len() > suffix_len {
                    domain.split_at(domain.len() - suffix_len).0
                } else {
                    ""
                };
                for (regex, value) in rules {
                    if regex.is_match(prefix) {
                        return Some(value);
                    }
                }
            }
        }
        for (regex_set, value) in &self.full_match {
            if regex_set.is_match(domain) {
                return Some(value);
            }
        }
        None
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
          rule:
            parent: example.net
            regex: abc.*
        - next: escaper_2
          rules:
            - parent: example.net
              regex: cde.+
            - .*[.]example[.]org
        "#;

        let v = YamlLoader::load_from_str(conf).unwrap();
        let mut builder = RegexMatchBuilder::default();
        builder.set_by_yaml(&v[0]).unwrap();

        let mut value_map = BTreeMap::new();
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_1") }, "escaper_1");
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_2") }, "escaper_2");
        let regex_match = builder.build(&value_map).unwrap();

        assert!(regex_match.check_domain("example.net").is_none());
        let value = *regex_match.check_domain("abc.example.net").unwrap();
        assert!(value.eq("escaper_1"));
        assert!(regex_match.check_domain("abcexample.net").is_none());
        let value = *regex_match.check_domain("cde1.example.net").unwrap();
        assert!(value.eq("escaper_2"));
        assert!(regex_match.check_domain("cde.example.net").is_none());
        let value = *regex_match.check_domain("a.example.org").unwrap();
        assert!(value.eq("escaper_2"));
    }

    #[test]
    fn yaml_map() {
        let conf = r#"
        escaper_1:
          parent: example.net
          regex: abc.*
        escaper_2:
          - parent: example.net
            regex: cde.+
          - .*[.]example[.]org
        "#;

        let v = YamlLoader::load_from_str(conf).unwrap();
        let mut builder = RegexMatchBuilder::default();
        builder.set_by_yaml(&v[0]).unwrap();

        let mut value_map = BTreeMap::new();
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_1") }, "escaper_1");
        value_map.insert(unsafe { NodeName::new_unchecked("escaper_2") }, "escaper_2");
        let regex_match = builder.build(&value_map).unwrap();

        assert!(regex_match.check_domain("example.net").is_none());
        let value = *regex_match.check_domain("abc.example.net").unwrap();
        assert!(value.eq("escaper_1"));
        assert!(regex_match.check_domain("abcexample.net").is_none());
        let value = *regex_match.check_domain("cde1.example.net").unwrap();
        assert!(value.eq("escaper_2"));
        assert!(regex_match.check_domain("cde.example.net").is_none());
        let value = *regex_match.check_domain("a.example.org").unwrap();
        assert!(value.eq("escaper_2"));
    }
}
