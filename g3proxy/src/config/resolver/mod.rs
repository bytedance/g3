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

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use indexmap::IndexSet;
use yaml_rust::{yaml, Yaml};

use g3_daemon::config::sort_nodes_in_dependency_graph;
use g3_yaml::{HybridParser, YamlDocPosition};

#[cfg(feature = "c-ares")]
pub(crate) mod c_ares;
pub(crate) mod trust_dns;

pub(crate) mod deny_all;
pub(crate) mod fail_over;

mod config;

pub(crate) use config::{AnyResolverConfig, ResolverConfig, ResolverConfigDiffAction};

use config::{CONFIG_KEY_RESOLVER_NAME, CONFIG_KEY_RESOLVER_TYPE};

mod registry;
pub(crate) use registry::clear;

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, crate::config::config_file_extension());
    parser.foreach_map(v, &|map, position| {
        let resolver = load_resolver(map, position)?;
        if let Some(old) = registry::add(resolver) {
            Err(anyhow!(
                "resolver with name {} has already been added",
                old.name()
            ))
        } else {
            Ok(())
        }
    })?;
    check_dependency()?;
    Ok(())
}

pub(crate) fn load_at_position(position: &YamlDocPosition) -> anyhow::Result<AnyResolverConfig> {
    let doc = g3_yaml::load_doc(position)?;
    if let Yaml::Hash(map) = doc {
        let resolver = load_resolver(&map, Some(position.clone()))?;
        let old_resolver = registry::add(resolver.clone());
        if let Err(e) = check_dependency() {
            // rollback
            match old_resolver {
                Some(resolver) => {
                    registry::add(resolver);
                }
                None => registry::del(resolver.name()),
            }
            return Err(e);
        }
        Ok(resolver)
    } else {
        Err(anyhow!("yaml doc {position} is not a map"))
    }
}

fn load_resolver(
    map: &yaml::Hash,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<AnyResolverConfig> {
    let resolver_type = g3_yaml::hash_get_required_str(map, CONFIG_KEY_RESOLVER_TYPE)?;
    match g3_yaml::key::normalize(resolver_type).as_str() {
        #[cfg(feature = "c-ares")]
        "c_ares" | "cares" => {
            let resolver = c_ares::CAresResolverConfig::parse(map, position)
                .context("failed to load this c-ares resolver")?;
            Ok(AnyResolverConfig::CAres(resolver))
        }
        "trust_dns" | "trustdns" => {
            let resolver = trust_dns::TrustDnsResolverConfig::parse(map, position)
                .context("failed to load this trust-dns resolver")?;
            Ok(AnyResolverConfig::TrustDns(resolver))
        }
        "deny_all" | "denyall" => {
            let resolver = deny_all::DenyAllResolverConfig::parse(map, position)
                .context("failed to load this DenyAll resolver")?;
            Ok(AnyResolverConfig::DenyAll(resolver))
        }
        "fail_over" | "failover" => {
            let resolver = fail_over::FailOverResolverConfig::parse(map, position)
                .context("failed to load this FailOver resolver")?;
            Ok(AnyResolverConfig::FailOver(resolver))
        }
        _ => Err(anyhow!("unsupported resolver type {resolver_type}")),
    }
}

fn get_edges_for_dependency_graph(
    all_config: &[Arc<AnyResolverConfig>],
    all_names: &IndexSet<String>,
) -> anyhow::Result<Vec<(usize, usize)>> {
    let mut edges: Vec<(usize, usize)> = Vec::with_capacity(all_config.len());

    // the isolated nodes is not added in edges
    for conf in all_config.iter() {
        let this_name = conf.name();
        let this_index = all_names.get_full(this_name).map(|x| x.0).unwrap();
        if let Some(names) = conf.dependent_resolver() {
            for peer_name in names {
                if let Some(r) = all_names.get_full(&peer_name) {
                    let peer_index = r.0;
                    edges.push((this_index, peer_index));
                } else {
                    return Err(anyhow!(
                        "escaper {this_name} dependent on {peer_name}, which is not existed"
                    ));
                }
            }
        }
    }

    Ok(edges)
}

pub(crate) fn get_all_sorted() -> anyhow::Result<Vec<Arc<AnyResolverConfig>>> {
    let all_config = registry::get_all();
    let mut all_names = IndexSet::<String>::new();
    let mut map_config = BTreeMap::<usize, Arc<AnyResolverConfig>>::new();

    for conf in all_config.iter() {
        let (index, ok) = all_names.insert_full(conf.name().to_string());
        assert!(ok);
        map_config.insert(index, Arc::clone(conf));
    }

    let edges = get_edges_for_dependency_graph(&all_config, &all_names)?;
    let mut nodes = sort_nodes_in_dependency_graph(edges).map_err(|node_index| {
        let name = all_names
            .get_index(node_index)
            .map(|x| x.to_string())
            .unwrap_or_else(|| "invalid node".to_string());
        anyhow!("Cycle detected in dependency for resolver {name}")
    })?;
    nodes.reverse();

    let mut all_config = Vec::<Arc<AnyResolverConfig>>::new();
    for node_index in 0usize..all_names.len() {
        // add isolated nodes first
        if !nodes.contains(&node_index) {
            all_config.push(map_config.remove(&node_index).unwrap());
        }
    }
    for node_index in nodes {
        // add connected nodes in order
        all_config.push(map_config.remove(&node_index).unwrap());
    }
    Ok(all_config)
}

fn check_dependency() -> anyhow::Result<()> {
    let all_config = registry::get_all();
    let mut all_names = IndexSet::<String>::new();

    for conf in all_config.iter() {
        all_names.insert(conf.name().to_string());
    }

    let edges = get_edges_for_dependency_graph(&all_config, &all_names)?;

    if let Err(node_index) = sort_nodes_in_dependency_graph(edges) {
        let name = all_names
            .get_index(node_index)
            .map(|x| x.to_string())
            .unwrap_or_else(|| "invalid node".to_string());
        Err(anyhow!("Cycle detected in dependency for resolver {name}"))
    } else {
        Ok(())
    }
}
