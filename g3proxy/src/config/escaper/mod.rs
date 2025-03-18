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

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use anyhow::anyhow;
use slog::Logger;
use yaml_rust::{Yaml, yaml};

use g3_daemon::config::TopoMap;
use g3_macros::AnyConfig;
use g3_types::metrics::NodeName;
use g3_types::net::{TcpConnectConfig, TcpSockSpeedLimitConfig, UdpSockSpeedLimitConfig};
use g3_yaml::{HybridParser, YamlDocPosition};

pub(crate) mod comply_audit;
pub(crate) mod direct_fixed;
pub(crate) mod direct_float;
pub(crate) mod divert_tcp;
pub(crate) mod dummy_deny;
pub(crate) mod proxy_float;
pub(crate) mod proxy_http;
pub(crate) mod proxy_https;
pub(crate) mod proxy_socks5;
pub(crate) mod proxy_socks5s;
pub(crate) mod route_client;
pub(crate) mod route_failover;
pub(crate) mod route_geoip;
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
    fn name(&self) -> &NodeName;
    fn position(&self) -> Option<YamlDocPosition>;
    fn escaper_type(&self) -> &str;
    fn resolver(&self) -> &NodeName;

    fn diff_action(&self, new: &AnyEscaperConfig) -> EscaperConfigDiffAction;

    fn dependent_escaper(&self) -> Option<BTreeSet<NodeName>> {
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

#[derive(Clone, AnyConfig)]
#[def_fn(name, &NodeName)]
#[def_fn(position, Option<YamlDocPosition>)]
#[def_fn(dependent_escaper, Option<BTreeSet<NodeName>>)]
#[def_fn(resolver, &NodeName)]
#[def_fn(diff_action, &Self, EscaperConfigDiffAction)]
pub(crate) enum AnyEscaperConfig {
    ComplyAudit(comply_audit::ComplyAuditEscaperConfig),
    DirectFixed(Box<direct_fixed::DirectFixedEscaperConfig>),
    DirectFloat(Box<direct_float::DirectFloatEscaperConfig>),
    DivertTcp(divert_tcp::DivertTcpEscaperConfig),
    DummyDeny(dummy_deny::DummyDenyEscaperConfig),
    ProxyFloat(proxy_float::ProxyFloatEscaperConfig),
    ProxyHttp(Box<proxy_http::ProxyHttpEscaperConfig>),
    ProxyHttps(Box<proxy_https::ProxyHttpsEscaperConfig>),
    ProxySocks5(proxy_socks5::ProxySocks5EscaperConfig),
    ProxySocks5s(proxy_socks5s::ProxySocks5sEscaperConfig),
    RouteFailover(route_failover::RouteFailoverEscaperConfig),
    RouteResolved(route_resolved::RouteResolvedEscaperConfig),
    RouteGeoIp(route_geoip::RouteGeoIpEscaperConfig),
    RouteMapping(route_mapping::RouteMappingEscaperConfig),
    RouteQuery(route_query::RouteQueryEscaperConfig),
    RouteSelect(route_select::RouteSelectEscaperConfig),
    RouteUpstream(route_upstream::RouteUpstreamEscaperConfig),
    RouteClient(route_client::RouteClientEscaperConfig),
    TrickFloat(trick_float::TrickFloatEscaperConfig),
}

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, g3_daemon::opts::config_file_extension());
    parser.foreach_map(v, |map, position| {
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
    build_topology_map()?;
    Ok(())
}

pub(crate) fn load_at_position(position: &YamlDocPosition) -> anyhow::Result<AnyEscaperConfig> {
    let doc = g3_yaml::load_doc(position)?;
    if let Yaml::Hash(map) = doc {
        let escaper = load_escaper(&map, Some(position.clone()))?;
        let old_escaper = registry::add(escaper.clone());
        if let Err(e) = build_topology_map() {
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
        "comply_audit" | "complyaudit" => {
            let config = comply_audit::ComplyAuditEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::ComplyAudit(config))
        }
        "direct_fixed" | "directfixed" => {
            let config = direct_fixed::DirectFixedEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::DirectFixed(Box::new(config)))
        }
        "direct_float" | "directfloat" | "direct_dynamic" | "directdynamic" => {
            let config = direct_float::DirectFloatEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::DirectFloat(Box::new(config)))
        }
        "divert_tcp" | "diverttcp" => {
            let config = divert_tcp::DivertTcpEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::DivertTcp(config))
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
        "proxy_socks5s" | "proxysocks5s" => {
            let config = proxy_socks5s::ProxySocks5sEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::ProxySocks5s(config))
        }
        "proxy_float" | "proxyfloat" | "proxy_dynamic" | "proxydynamic" => {
            let config = proxy_float::ProxyFloatEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::ProxyFloat(config))
        }
        "route_failover" | "routefailover" => {
            let config = route_failover::RouteFailoverEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::RouteFailover(config))
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
        "route_geoip" | "routegeoip" | "route_geo_ip" => {
            let config = route_geoip::RouteGeoIpEscaperConfig::parse(map, position)?;
            Ok(AnyEscaperConfig::RouteGeoIp(config))
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

fn build_topology_map() -> anyhow::Result<TopoMap> {
    let mut topo_map = TopoMap::default();

    for name in registry::get_all_names() {
        topo_map.add_node(&name, &|name| {
            let conf = registry::get(name)?;
            conf.dependent_escaper()
        })?;
    }

    Ok(topo_map)
}

pub(crate) fn get_all_sorted() -> anyhow::Result<Vec<Arc<AnyEscaperConfig>>> {
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
