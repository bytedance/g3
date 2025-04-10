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

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_daemon::config::TopoMap;
use g3_macros::AnyConfig;
use g3_types::metrics::NodeName;
use g3_yaml::{HybridParser, YamlDocPosition};

mod registry;
pub(crate) use registry::clear;

pub(crate) mod aggregate;
pub(crate) mod discard;
pub(crate) mod internal;
pub(crate) mod regulate;

const CONFIG_KEY_COLLECTOR_TYPE: &str = "type";
const CONFIG_KEY_COLLECTOR_NAME: &str = "name";

pub(crate) enum CollectorConfigDiffAction {
    NoAction,
    SpawnNew,
    Reload,
    Update,
}

pub(crate) trait CollectorConfig {
    fn name(&self) -> &NodeName;
    fn position(&self) -> Option<YamlDocPosition>;
    fn collector_type(&self) -> &'static str;

    fn diff_action(&self, new: &AnyCollectorConfig) -> CollectorConfigDiffAction;

    fn dependent_collector(&self) -> Option<BTreeSet<NodeName>> {
        None
    }
}

#[derive(Clone, Debug, AnyConfig)]
#[def_fn(name, &NodeName)]
#[def_fn(position, Option<YamlDocPosition>)]
#[def_fn(collector_type, &'static str)]
#[def_fn(dependent_collector, Option<BTreeSet<NodeName>>)]
#[def_fn(diff_action, &Self, CollectorConfigDiffAction)]
pub(crate) enum AnyCollectorConfig {
    Aggregate(aggregate::AggregateCollectorConfig),
    Discard(discard::DiscardCollectorConfig),
    Internal(internal::InternalCollectorConfig),
    Regulate(regulate::RegulateCollectorConfig),
}

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, g3_daemon::opts::config_file_extension());
    parser.foreach_map(v, |map, position| {
        let collector = load_collector(map, position)?;
        if let Some(old_collector) = registry::add(collector) {
            Err(anyhow!(
                "collector with name {} already exists",
                old_collector.name()
            ))
        } else {
            Ok(())
        }
    })?;
    build_topology_map()?;
    Ok(())
}

pub(crate) fn load_at_position(position: &YamlDocPosition) -> anyhow::Result<AnyCollectorConfig> {
    let doc = g3_yaml::load_doc(position)?;
    if let Yaml::Hash(map) = doc {
        let collector = load_collector(&map, Some(position.clone()))?;
        let old_collector = registry::add(collector.clone());
        if let Err(e) = build_topology_map() {
            // rollback
            match old_collector {
                Some(collector) => {
                    registry::add(collector);
                }
                None => registry::del(collector.name()),
            }
            Err(e)
        } else {
            Ok(collector)
        }
    } else {
        Err(anyhow!("yaml doc {position} is not a map"))
    }
}

fn load_collector(
    map: &yaml::Hash,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<AnyCollectorConfig> {
    let collector_type = g3_yaml::hash_get_required_str(map, CONFIG_KEY_COLLECTOR_TYPE)?;
    match g3_yaml::key::normalize(collector_type).as_str() {
        "discard" => {
            let collector = discard::DiscardCollectorConfig::parse(map, position)
                .context("failed to load this Discard collector")?;
            Ok(AnyCollectorConfig::Discard(collector))
        }
        "internal" => {
            let collector = internal::InternalCollectorConfig::parse(map, position)
                .context("failed to load this Internal collector")?;
            Ok(AnyCollectorConfig::Internal(collector))
        }
        "regulate" => {
            let collector = regulate::RegulateCollectorConfig::parse(map, position)
                .context("failed to load this Regulate collector")?;
            Ok(AnyCollectorConfig::Regulate(collector))
        }
        "aggregate" => {
            let collector = aggregate::AggregateCollectorConfig::parse(map, position)
                .context("failed to load this Aggregate collector")?;
            Ok(AnyCollectorConfig::Aggregate(collector))
        }
        _ => Err(anyhow!("unsupported collector type {}", collector_type)),
    }
}

fn build_topology_map() -> anyhow::Result<TopoMap> {
    let mut topo_map = TopoMap::default();

    for name in registry::get_all_names() {
        topo_map.add_node(&name, &|name| {
            let conf = registry::get(name)?;
            conf.dependent_collector()
        })?;
    }

    Ok(topo_map)
}

pub(crate) fn get_all_sorted() -> anyhow::Result<Vec<Arc<AnyCollectorConfig>>> {
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
