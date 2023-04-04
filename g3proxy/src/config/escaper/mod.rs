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

use anyhow::anyhow;
use indexmap::IndexSet;
use slog::Logger;
use yaml_rust::{yaml, Yaml};

use g3_daemon::config::sort_nodes_in_dependency_graph;
use g3_types::metrics::MetricsName;
use g3_types::net::{TcpConnectConfig, TcpSockSpeedLimitConfig, UdpSockSpeedLimitConfig};
use g3_yaml::{HybridParser, YamlDocPosition};

pub(crate) mod direct_fixed;
pub(crate) mod direct_float;
pub(crate) mod dummy_deny;
pub(crate) mod proxy_float;
pub(crate) mod proxy_http;
pub(crate) mod proxy_https;
pub(crate) mod proxy_socks5;
pub(crate) mod route_client;
pub(crate) mod route_mapping;
pub(crate) mod route_query;
pub(crate) mod route_resolved;
pub(crate) mod route_select;
pub(crate) mod route_upstream;
pub(crate) mod trick_float;

mod registry;
pub(crate) use registry::clear;

mod verify;
use verify::EscaperConfigVerifier;

const CONFIG_KEY_ESCAPER_TYPE: &str = "type";
const CONFIG_KEY_ESCAPER_NAME: &str = "name";

pub(crate) enum EscaperConfigDiffAction {
    NoAction,
    SpawnNew,
    Reload,
    #[allow(unused)]
    UpdateInPlace(u64), // to support escaper custom hot update, take a flags param
}

pub(crate) trait EscaperConfig {
    fn name(&self) -> &str;
    fn position(&self) -> Option<YamlDocPosition>;
    fn escaper_type(&self) -> &str;
    fn resolver(&self) -> &MetricsName;

    fn diff_action(&self, new: &AnyEscaperConfig) -> EscaperConfigDiffAction;

    fn dependent_escaper(&self) -> Option<BTreeSet<String>> {
        None
    }
    fn shared_logger(&self) -> Option<&str> {
        None
    }
    fn get_escape_logger(&self) -> Logger {
        if let Some(shared_logger) = self.shared_logger() {
            crate::log::escape::get_shared_logger(shared_logger, self.escaper_type(), self.name())
        } else {
            crate::log::escape::get_logger(self.escaper_type(), self.name())
        }
    }
}

#[derive(Clone, Default, Eq, PartialEq)]
pub(crate) struct GeneralEscaperConfig {
    pub(crate) tcp_sock_speed_limit: TcpSockSpeedLimitConfig,
    pub(crate) udp_sock_speed_limit: UdpSockSpeedLimitConfig,
    pub(crate) tcp_connect: TcpConnectConfig,
}

#[derive(Clone)]
pub(crate) enum AnyEscaperConfig {
    DirectFixed(Box<direct_fixed::DirectFixedEscaperConfig>),
    DirectFloat(Box<direct_float::DirectFloatEscaperConfig>),
    DummyDeny(dummy_deny::DummyDenyEscaperConfig),
    ProxyFloat(proxy_float::ProxyFloatEscaperConfig),
    ProxyHttp(Box<proxy_http::ProxyHttpEscaperConfig>),
    ProxyHttps(Box<proxy_https::ProxyHttpsEscaperConfig>),
    ProxySocks5(proxy_socks5::ProxySocks5EscaperConfig),
    RouteResolved(route_resolved::RouteResolvedEscaperConfig),
    RouteMapping(route_mapping::RouteMappingEscaperConfig),
    RouteQuery(route_query::RouteQueryEscaperConfig),
    RouteSelect(route_select::RouteSelectEscaperConfig),
    RouteUpstream(route_upstream::RouteUpstreamEscaperConfig),
    RouteClient(route_client::RouteClientEscaperConfig),
    TrickFloat(trick_float::TrickFloatEscaperConfig),
}

macro_rules! impl_transparent0 {
    ($f:tt, $v:ty) => {
        pub(crate) fn $f(&self) -> $v {
            match self {
                AnyEscaperConfig::DirectFixed(s) => s.$f(),
                AnyEscaperConfig::DirectFloat(s) => s.$f(),
                AnyEscaperConfig::DummyDeny(s) => s.$f(),
                AnyEscaperConfig::ProxyFloat(s) => s.$f(),
                AnyEscaperConfig::ProxyHttp(s) => s.$f(),
                AnyEscaperConfig::ProxyHttps(s) => s.$f(),
                AnyEscaperConfig::ProxySocks5(s) => s.$f(),
                AnyEscaperConfig::RouteResolved(s) => s.$f(),
                AnyEscaperConfig::RouteMapping(s) => s.$f(),
                AnyEscaperConfig::RouteQuery(s) => s.$f(),
                AnyEscaperConfig::RouteSelect(s) => s.$f(),
                AnyEscaperConfig::RouteUpstream(s) => s.$f(),
                AnyEscaperConfig::RouteClient(s) => s.$f(),
                AnyEscaperConfig::TrickFloat(s) => s.$f(),
            }
        }
    };
}

macro_rules! impl_transparent1 {
    ($f:tt, $v:ty, $p:ty) => {
        pub(crate) fn $f(&self, p: $p) -> $v {
            match self {
                AnyEscaperConfig::DirectFixed(s) => s.$f(p),
                AnyEscaperConfig::DirectFloat(s) => s.$f(p),
                AnyEscaperConfig::DummyDeny(s) => s.$f(p),
                AnyEscaperConfig::ProxyFloat(s) => s.$f(p),
                AnyEscaperConfig::ProxyHttp(s) => s.$f(p),
                AnyEscaperConfig::ProxyHttps(s) => s.$f(p),
                AnyEscaperConfig::ProxySocks5(s) => s.$f(p),
                AnyEscaperConfig::RouteResolved(s) => s.$f(p),
                AnyEscaperConfig::RouteMapping(s) => s.$f(p),
                AnyEscaperConfig::RouteQuery(s) => s.$f(p),
                AnyEscaperConfig::RouteSelect(s) => s.$f(p),
                AnyEscaperConfig::RouteUpstream(s) => s.$f(p),
                AnyEscaperConfig::RouteClient(s) => s.$f(p),
                AnyEscaperConfig::TrickFloat(s) => s.$f(p),
            }
        }
    };
}

impl AnyEscaperConfig {
    impl_transparent0!(name, &str);
    impl_transparent0!(position, Option<YamlDocPosition>);
    impl_transparent0!(dependent_escaper, Option<BTreeSet<String>>);
    impl_transparent0!(resolver, &MetricsName);

    impl_transparent1!(diff_action, EscaperConfigDiffAction, &Self);
}

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, crate::config::config_file_extension());
    parser.foreach_map(v, &|map, position| {
        let escaper = load_escaper(map, position)?;
        if let Some(old_escaper) = registry::add(escaper) {
            Err(anyhow!(
                "escaper with name {} already exists",
                old_escaper.name()
            ))
        } else {
            Ok(())
        }
    })?;
    check_dependency()?;
    Ok(())
}

pub(crate) fn load_at_position(position: &YamlDocPosition) -> anyhow::Result<AnyEscaperConfig> {
    let doc = g3_yaml::load_doc(position)?;
    if let Yaml::Hash(map) = doc {
        let escaper = load_escaper(&map, Some(position.clone()))?;
        let old_escaper = registry::add(escaper.clone());
        if let Err(e) = check_dependency() {
            // rollback
            match old_escaper {
                Some(escaper) => {
                    registry::add(escaper);
                }
                None => registry::del(escaper.name()),
            }
            Err(e)
        } else {
            Ok(escaper)
        }
    } else {
        Err(anyhow!("yaml doc {position} is not a map"))
    }
}

fn load_escaper(
    map: &yaml::Hash,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<AnyEscaperConfig> {
    let escaper_type = g3_yaml::hash_get_required_str(map, CONFIG_KEY_ESCAPER_TYPE)?;
    match g3_yaml::key::normalize(escaper_type).as_str() {
        "direct_fixed" | "directfixed" => {
            let config = direct_fixed::DirectFixedEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::DirectFixed(Box::new(config)))
        }
        "direct_float" | "directfloat" | "direct_dynamic" | "directdynamic" => {
            let config = direct_float::DirectFloatEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::DirectFloat(Box::new(config)))
        }
        "dummy_deny" | "dummydeny" => {
            let config = dummy_deny::DummyDenyEscaperConfig::parse(map, position, None)?;
            Ok(AnyEscaperConfig::DummyDeny(config))
        }
        "proxy_http" | "proxyhttp" => {
            let config = proxy_http::ProxyHttpEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::ProxyHttp(Box::new(config)))
        }
        "proxy_https" | "proxyhttps" => {
            let config = proxy_https::ProxyHttpsEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::ProxyHttps(Box::new(config)))
        }
        "proxy_socks5" | "proxysocks5" => {
            let config = proxy_socks5::ProxySocks5EscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::ProxySocks5(config))
        }
        "proxy_float" | "proxyfloat" | "proxy_dynamic" | "proxydynamic" => {
            let config = proxy_float::ProxyFloatEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::ProxyFloat(config))
        }
        "route_mapping" | "routemapping" => {
            let config = route_mapping::RouteMappingEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::RouteMapping(config))
        }
        "route_query" | "routequery" => {
            let config = route_query::RouteQueryEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::RouteQuery(config))
        }
        "route_resolved" | "routeresolved" | "route_dst_ip" | "route_dstip" | "routedstip" => {
            let config = route_resolved::RouteResolvedEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::RouteResolved(config))
        }
        "route_select" | "routeselect" => {
            let config = route_select::RouteSelectEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::RouteSelect(config))
        }
        "route_upstream" | "routeupstream" => {
            let config = route_upstream::RouteUpstreamEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::RouteUpstream(config))
        }
        "route_client" | "routeclient" => {
            let config = route_client::RouteClientEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::RouteClient(config))
        }
        "trick_float" | "trickfloat" => {
            let config = trick_float::TrickFloatEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::TrickFloat(config))
        }
        _ => Err(anyhow!("unsupported escaper type {escaper_type}")),
    }
}

fn get_edges_for_dependency_graph(
    all_config: &[Arc<AnyEscaperConfig>],
    all_names: &IndexSet<String>,
) -> anyhow::Result<Vec<(usize, usize)>> {
    let mut edges: Vec<(usize, usize)> = Vec::with_capacity(all_config.len());

    // the isolated nodes is not added in edges
    for conf in all_config.iter() {
        let this_name = conf.name();
        let this_index = all_names.get_full(this_name).map(|x| x.0).unwrap();
        if let Some(names) = conf.dependent_escaper() {
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

pub(crate) fn get_all_sorted() -> anyhow::Result<Vec<Arc<AnyEscaperConfig>>> {
    let all_config = registry::get_all();
    let mut all_names = IndexSet::<String>::new();
    let mut map_config = BTreeMap::<usize, Arc<AnyEscaperConfig>>::new();

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
        anyhow!("Cycle detected in dependency for escaper {name}")
    })?;
    nodes.reverse();

    let mut all_config = Vec::<Arc<AnyEscaperConfig>>::new();
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
        Err(anyhow!("Cycle detected in dependency for escaper {name}"))
    } else {
        Ok(())
    }
}
