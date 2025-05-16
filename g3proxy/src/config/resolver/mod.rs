/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_daemon::config::TopoMap;
use g3_macros::AnyConfig;
use g3_types::metrics::NodeName;
use g3_yaml::{HybridParser, YamlDocPosition};

#[cfg(feature = "c-ares")]
pub(crate) mod c_ares;
pub(crate) mod hickory;

pub(crate) mod deny_all;
pub(crate) mod fail_over;

mod registry;
pub(crate) use registry::clear;

const CONFIG_KEY_RESOLVER_TYPE: &str = "type";
const CONFIG_KEY_RESOLVER_NAME: &str = "name";

pub(crate) enum ResolverConfigDiffAction {
    NoAction,
    SpawnNew,
    Update,
}

pub(crate) trait ResolverConfig {
    fn name(&self) -> &NodeName;
    fn position(&self) -> Option<YamlDocPosition>;
    fn r#type(&self) -> &'static str;

    fn diff_action(&self, new: &AnyResolverConfig) -> ResolverConfigDiffAction;
    fn dependent_resolver(&self) -> Option<BTreeSet<NodeName>>;
}

#[derive(Clone, AnyConfig)]
#[def_fn(name, &NodeName)]
#[def_fn(position, Option<YamlDocPosition>)]
#[def_fn(dependent_resolver, Option<BTreeSet<NodeName>>)]
#[def_fn(diff_action, &Self, ResolverConfigDiffAction)]
pub(crate) enum AnyResolverConfig {
    #[cfg(feature = "c-ares")]
    CAres(c_ares::CAresResolverConfig),
    Hickory(Box<hickory::HickoryResolverConfig>),
    DenyAll(deny_all::DenyAllResolverConfig),
    FailOver(fail_over::FailOverResolverConfig),
}

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, g3_daemon::opts::config_file_extension());
    parser.foreach_map(v, |map, position| {
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
    build_topology_map()?;
    Ok(())
}

pub(crate) fn load_at_position(position: &YamlDocPosition) -> anyhow::Result<AnyResolverConfig> {
    let doc = g3_yaml::load_doc(position)?;
    if let Yaml::Hash(map) = doc {
        let resolver = load_resolver(&map, Some(position.clone()))?;
        let old_resolver = registry::add(resolver.clone());
        if let Err(e) = build_topology_map() {
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
        "hickory" | "hickory_dns" | "hickorydns" | "trust_dns" | "trustdns" => {
            let resolver = hickory::HickoryResolverConfig::parse(map, position)
                .context("failed to load this hickory resolver")?;
            Ok(AnyResolverConfig::Hickory(Box::new(resolver)))
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

fn build_topology_map() -> anyhow::Result<TopoMap> {
    let mut topo_map = TopoMap::default();

    for name in registry::get_all_names() {
        topo_map.add_node(&name, &|name| {
            let conf = registry::get(name)?;
            conf.dependent_resolver()
        })?;
    }

    Ok(topo_map)
}

pub(crate) fn get_all_sorted() -> anyhow::Result<Vec<Arc<AnyResolverConfig>>> {
    let topo_map = build_topology_map()?;
    let sorted_nodes = topo_map.sorted_nodes();
    let mut sorted_conf = Vec::with_capacity(sorted_nodes.len());
    for node in sorted_nodes {
        let Some(conf) = registry::get(node.name()) else {
            continue;
        };
        sorted_conf.push(conf);
    }
    Ok(sorted_conf)
}
