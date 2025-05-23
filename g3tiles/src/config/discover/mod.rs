/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_macros::AnyConfig;
use g3_types::metrics::NodeName;
use g3_yaml::{HybridParser, YamlDocPosition};

mod registry;
pub(crate) use registry::{clear, get_all};

pub(crate) mod host_resolver;
pub(crate) mod static_addr;

const CONFIG_KEY_DISCOVER_TYPE: &str = "type";
const CONFIG_KEY_DISCOVER_NAME: &str = "name";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DiscoverRegisterData {
    Null,
    Yaml(Yaml),
    #[allow(unused)]
    Json(serde_json::Value),
}

pub(crate) enum DiscoverConfigDiffAction {
    NoAction,
    SpawnNew,
    #[allow(unused)]
    UpdateInPlace,
}

pub(crate) trait DiscoverConfig {
    fn name(&self) -> &NodeName;
    fn position(&self) -> Option<YamlDocPosition>;
    fn r#type(&self) -> &'static str;

    fn diff_action(&self, new: &AnyDiscoverConfig) -> DiscoverConfigDiffAction;
}

#[derive(Clone, AnyConfig)]
#[def_fn(name, &NodeName)]
#[def_fn(r#type, &'static str)]
#[def_fn(position, Option<YamlDocPosition>)]
#[def_fn(diff_action, &Self, DiscoverConfigDiffAction)]
pub(crate) enum AnyDiscoverConfig {
    StaticAddr(static_addr::StaticAddrDiscoverConfig),
    HostResolver(host_resolver::HostResolverDiscoverConfig),
}

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, g3_daemon::opts::config_file_extension());
    parser.foreach_map(v, |map, position| {
        let site = load_discover(map, position)?;
        registry::add(site, false)?;
        Ok(())
    })?;
    Ok(())
}

pub(crate) fn load_at_position(position: &YamlDocPosition) -> anyhow::Result<AnyDiscoverConfig> {
    let doc = g3_yaml::load_doc(position)?;
    if let Yaml::Hash(map) = doc {
        let site = load_discover(&map, Some(position.clone()))?;
        registry::add(site.clone(), true)?;
        Ok(site)
    } else {
        Err(anyhow!("yaml doc {position} is not a map"))
    }
}

fn load_discover(
    map: &yaml::Hash,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<AnyDiscoverConfig> {
    let discover_type = g3_yaml::hash_get_required_str(map, CONFIG_KEY_DISCOVER_TYPE)?;
    match g3_yaml::key::normalize(discover_type).as_str() {
        "static_addr" | "staticaddr" => {
            let discover = static_addr::StaticAddrDiscoverConfig::parse_yaml_conf(map, position)
                .context("failed to load this StaticAddr discover")?;
            Ok(AnyDiscoverConfig::StaticAddr(discover))
        }
        "host_resolver" | "hostresolver" => {
            let discover =
                host_resolver::HostResolverDiscoverConfig::parse_yaml_conf(map, position)
                    .context("failed to load this HostResolver discover")?;
            Ok(AnyDiscoverConfig::HostResolver(discover))
        }
        _ => Err(anyhow!("unsupported discover type {}", discover_type)),
    }
}
