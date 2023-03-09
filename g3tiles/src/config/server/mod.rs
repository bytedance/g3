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

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use indexmap::IndexSet;
use slog::Logger;
use yaml_rust::{yaml, Yaml};

use g3_daemon::config::sort_nodes_in_dependency_graph;
use g3_yaml::{HybridParser, YamlDocPosition};

pub(crate) mod dummy_close;
pub(crate) mod plain_tcp_port;

pub(crate) mod openssl_proxy;
pub(crate) mod rustls_proxy;

mod registry;

pub(crate) use registry::clear;

const CONFIG_KEY_SERVER_TYPE: &str = "type";
const CONFIG_KEY_SERVER_NAME: &str = "name";

const IDLE_CHECK_MAXIMUM_DURATION: Duration = Duration::from_secs(1800);
const IDLE_CHECK_DEFAULT_DURATION: Duration = Duration::from_secs(300);

pub(crate) enum ServerConfigDiffAction {
    NoAction,
    SpawnNew,
    ReloadOnlyConfig,
    ReloadAndRespawn,
    #[allow(unused)]
    UpdateInPlace(u64), // to support server custom hot update, take a flags param
}

pub(crate) trait ServerConfig {
    fn name(&self) -> &str;
    fn position(&self) -> Option<YamlDocPosition>;
    fn server_type(&self) -> &'static str;

    fn diff_action(&self, new: &AnyServerConfig) -> ServerConfigDiffAction;

    fn dependent_server(&self) -> Option<BTreeSet<String>> {
        None
    }
    fn shared_logger(&self) -> Option<&str> {
        None
    }
    fn get_task_logger(&self) -> Logger {
        if let Some(shared_logger) = self.shared_logger() {
            crate::log::task::get_shared_logger(shared_logger, self.server_type(), self.name())
        } else {
            crate::log::task::get_logger(self.server_type(), self.name())
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum AnyServerConfig {
    DummyClose(dummy_close::DummyCloseServerConfig),
    PlainTcpPort(plain_tcp_port::PlainTcpPortConfig),
    OpensslProxy(openssl_proxy::OpensslProxyServerConfig),
    RustlsProxy(rustls_proxy::RustlsProxyServerConfig),
}

macro_rules! impl_transparent0 {
    ($f:tt, $v:ty) => {
        pub(crate) fn $f(&self) -> $v {
            match self {
                AnyServerConfig::DummyClose(s) => s.$f(),
                AnyServerConfig::PlainTcpPort(s) => s.$f(),
                AnyServerConfig::OpensslProxy(s) => s.$f(),
                AnyServerConfig::RustlsProxy(s) => s.$f(),
            }
        }
    };
}

macro_rules! impl_transparent1 {
    ($f:tt, $v:ty, $p:ty) => {
        pub(crate) fn $f(&self, p: $p) -> $v {
            match self {
                AnyServerConfig::DummyClose(s) => s.$f(p),
                AnyServerConfig::PlainTcpPort(s) => s.$f(p),
                AnyServerConfig::OpensslProxy(s) => s.$f(p),
                AnyServerConfig::RustlsProxy(s) => s.$f(p),
            }
        }
    };
}

impl AnyServerConfig {
    impl_transparent0!(name, &str);
    impl_transparent0!(position, Option<YamlDocPosition>);
    impl_transparent0!(server_type, &'static str);
    impl_transparent0!(dependent_server, Option<BTreeSet<String>>);

    impl_transparent1!(diff_action, ServerConfigDiffAction, &Self);
}

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, crate::config::config_file_extension());
    parser.foreach_map(v, &|map, position| {
        let server = load_server(map, position)?;
        registry::add(server, false)?;
        Ok(())
    })?;
    check_dependency()?;
    Ok(())
}

pub(crate) fn load_at_position(position: &YamlDocPosition) -> anyhow::Result<AnyServerConfig> {
    let doc = g3_yaml::load_doc(position)?;
    if let Yaml::Hash(map) = doc {
        let server = load_server(&map, Some(position.clone()))?;
        registry::add(server.clone(), true)?;
        Ok(server)
    } else {
        Err(anyhow!("yaml doc {position} is not a map"))
    }
}

fn load_server(
    map: &yaml::Hash,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<AnyServerConfig> {
    let server_type = g3_yaml::hash_get_required_str(map, CONFIG_KEY_SERVER_TYPE)?;
    match g3_yaml::key::normalize(server_type).as_str() {
        "dummy_close" | "dummyclose" => {
            let server = dummy_close::DummyCloseServerConfig::parse(map, position)
                .context("failed to load this DummyClose server")?;
            Ok(AnyServerConfig::DummyClose(server))
        }
        "plain_tcp_port" | "plaintcpport" | "plain_tcp" | "plaintcp" => {
            let server = plain_tcp_port::PlainTcpPortConfig::parse(map, position)
                .context("failed to load this PlainTcpPort server")?;
            Ok(AnyServerConfig::PlainTcpPort(server))
        }
        "openssl_proxy" | "opensslproxy" => {
            let server = openssl_proxy::OpensslProxyServerConfig::parse(map, position)
                .context("failed to load this OpensslProxy server")?;
            Ok(AnyServerConfig::OpensslProxy(server))
        }
        "rustls_proxy" | "rustlsproxy" => {
            let server = rustls_proxy::RustlsProxyServerConfig::parse(map, position)
                .context("failed to load this RustlsProxy server")?;
            Ok(AnyServerConfig::RustlsProxy(server))
        }
        _ => Err(anyhow!("unsupported server type {}", server_type)),
    }
}

fn get_edges_for_dependency_graph(
    all_config: &[Arc<AnyServerConfig>],
    all_names: &IndexSet<String>,
) -> anyhow::Result<Vec<(usize, usize)>> {
    let mut edges: Vec<(usize, usize)> = Vec::with_capacity(all_config.len());

    // the isolated nodes is not added in edges
    for conf in all_config.iter() {
        let this_name = conf.name();
        let this_index = all_names.get_full(this_name).map(|x| x.0).unwrap();
        if let Some(names) = conf.dependent_server() {
            for peer_name in names {
                if let Some(r) = all_names.get_full(&peer_name) {
                    let peer_index = r.0;
                    edges.push((this_index, peer_index));
                } else {
                    return Err(anyhow!(
                        "server {this_name} dependent on {peer_name}, which is not existed"
                    ));
                }
            }
        }
    }

    Ok(edges)
}

pub(crate) fn get_all_sorted() -> anyhow::Result<Vec<Arc<AnyServerConfig>>> {
    let all_config = registry::get_all();
    let mut all_names = IndexSet::<String>::new();
    let mut map_config = BTreeMap::<usize, Arc<AnyServerConfig>>::new();

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
        anyhow!("Cycle detected in dependency for server {name}")
    })?;
    nodes.reverse();

    let mut all_config = Vec::<Arc<AnyServerConfig>>::new();
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
        Err(anyhow!("Cycle detected in dependency for server {name}"))
    } else {
        Ok(())
    }
}
