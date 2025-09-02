/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow};
use slog::Logger;
use yaml_rust::{Yaml, yaml};

use g3_daemon::config::TopoMap;
use g3_io_ext::StreamCopyConfig;
use g3_macros::AnyConfig;
use g3_types::metrics::NodeName;
use g3_yaml::{HybridParser, YamlDocPosition};

use crate::audit::AuditHandle;
use crate::auth::UserGroup;

pub(crate) mod dummy_close;
pub(crate) mod intelli_proxy;
pub(crate) mod native_tls_port;
#[cfg(feature = "quic")]
pub(crate) mod plain_quic_port;
pub(crate) mod plain_tcp_port;
pub(crate) mod plain_tls_port;

pub(crate) mod http_proxy;
pub(crate) mod http_rproxy;
pub(crate) mod sni_proxy;
pub(crate) mod socks_proxy;
pub(crate) mod tcp_stream;
pub(crate) mod username_params_to_escaper;
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
))]
pub(crate) mod tcp_tproxy;
pub(crate) mod tls_stream;

mod registry;
pub(crate) use registry::clear;

const CONFIG_KEY_SERVER_TYPE: &str = "type";
const CONFIG_KEY_SERVER_NAME: &str = "name";

const IDLE_CHECK_MAXIMUM_DURATION: Duration = Duration::from_secs(1800);
const IDLE_CHECK_DEFAULT_DURATION: Duration = Duration::from_secs(60);
const IDLE_CHECK_DEFAULT_MAX_COUNT: usize = 5;

pub(crate) enum ServerConfigDiffAction {
    NoAction,
    SpawnNew,
    ReloadNoRespawn,
    ReloadAndRespawn,
    #[allow(unused)]
    UpdateInPlace(u64), // to support server custom hot update, take a flags param
}

pub(crate) trait ServerConfig {
    fn name(&self) -> &NodeName;
    fn position(&self) -> Option<YamlDocPosition>;
    fn r#type(&self) -> &'static str;

    fn escaper(&self) -> &NodeName;
    fn user_group(&self) -> &NodeName;
    fn auditor(&self) -> &NodeName;

    fn diff_action(&self, new: &AnyServerConfig) -> ServerConfigDiffAction;

    fn dependent_server(&self) -> Option<BTreeSet<NodeName>> {
        None
    }
    fn shared_logger(&self) -> Option<&str> {
        None
    }
    fn get_task_logger(&self) -> Option<Logger> {
        if let Some(shared_logger) = self.shared_logger() {
            crate::log::task::get_shared_logger(shared_logger, self.r#type(), self.name())
        } else {
            crate::log::task::get_logger(self.r#type(), self.name())
        }
    }

    fn task_log_flush_interval(&self) -> Option<Duration> {
        None
    }

    fn limited_copy_config(&self) -> StreamCopyConfig {
        StreamCopyConfig::default()
    }
    fn task_max_idle_count(&self) -> usize {
        1
    }

    fn get_user_group(&self) -> Option<Arc<UserGroup>> {
        if self.user_group().is_empty() {
            None
        } else {
            Some(crate::auth::get_or_insert_default(self.user_group()))
        }
    }

    fn get_audit_handle(&self) -> anyhow::Result<Option<Arc<AuditHandle>>> {
        if self.auditor().is_empty() {
            Ok(None)
        } else {
            let auditor = crate::audit::get_or_insert_default(self.auditor());
            let handle = auditor
                .build_handle()
                .context("failed to build audit handle")?;
            Ok(Some(handle))
        }
    }
}

#[derive(Clone, Debug, AnyConfig)]
#[def_fn(name, &NodeName)]
#[def_fn(position, Option<YamlDocPosition>)]
#[def_fn(r#type, &'static str)]
#[def_fn(dependent_server, Option<BTreeSet<NodeName>>)]
#[def_fn(escaper, &NodeName)]
#[def_fn(user_group, &NodeName)]
#[def_fn(auditor, &NodeName)]
#[def_fn(diff_action, &Self, ServerConfigDiffAction)]
pub(crate) enum AnyServerConfig {
    DummyClose(dummy_close::DummyCloseServerConfig),
    PlainTcpPort(plain_tcp_port::PlainTcpPortConfig),
    PlainTlsPort(plain_tls_port::PlainTlsPortConfig),
    NativeTlsPort(native_tls_port::NativeTlsPortConfig),
    #[cfg(feature = "quic")]
    PlainQuicPort(plain_quic_port::PlainQuicPortConfig),
    IntelliProxy(intelli_proxy::IntelliProxyConfig),
    TcpStream(tcp_stream::TcpStreamServerConfig),
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd"
    ))]
    TcpTProxy(tcp_tproxy::TcpTProxyServerConfig),
    TlsStream(tls_stream::TlsStreamServerConfig),
    SniProxy(sni_proxy::SniProxyServerConfig),
    SocksProxy(socks_proxy::SocksProxyServerConfig),
    HttpProxy(http_proxy::HttpProxyServerConfig),
    HttpRProxy(http_rproxy::HttpRProxyServerConfig),
}

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, g3_daemon::opts::config_file_extension());
    parser.foreach_map(v, |map, position| {
        let server = load_server(map, position)?;
        if let Some(old_server) = registry::add(server) {
            Err(anyhow!(
                "server with name {} already exists",
                old_server.name()
            ))
        } else {
            Ok(())
        }
    })?;
    build_topology_map()?;
    Ok(())
}

pub(crate) fn load_at_position(position: &YamlDocPosition) -> anyhow::Result<AnyServerConfig> {
    let doc = g3_yaml::load_doc(position)?;
    if let Yaml::Hash(map) = doc {
        let server = load_server(&map, Some(position.clone()))?;
        let old_server = registry::add(server.clone());
        if let Err(e) = build_topology_map() {
            // rollback
            match old_server {
                Some(server) => {
                    registry::add(server);
                }
                None => registry::del(server.name()),
            }
            Err(e)
        } else {
            Ok(server)
        }
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
        "plain_tls_port" | "plaintlsport" | "plain_tls" | "plaintls" => {
            let server = plain_tls_port::PlainTlsPortConfig::parse(map, position)
                .context("failed to load this PlainTlsPort server")?;
            Ok(AnyServerConfig::PlainTlsPort(server))
        }
        "native_tls_port" | "nativetlsport" | "native_tls" | "nativetls" => {
            let server = native_tls_port::NativeTlsPortConfig::parse(map, position)
                .context("failed to load this NativeTlsPort server")?;
            Ok(AnyServerConfig::NativeTlsPort(server))
        }
        #[cfg(feature = "quic")]
        "plain_quic_port" | "plainquicport" | "plain_quic" | "plainquic" => {
            let server = plain_quic_port::PlainQuicPortConfig::parse(map, position)
                .context("failed to load this PlainQuicPort server")?;
            Ok(AnyServerConfig::PlainQuicPort(server))
        }
        "intelli_proxy" | "intelliproxy" | "ppdp_tcp_port" | "ppdptcpport" | "ppdp_tcp"
        | "ppdptcp" => {
            let server = intelli_proxy::IntelliProxyConfig::parse(map, position)
                .context("failed to load this IntelliProxy server")?;
            Ok(AnyServerConfig::IntelliProxy(server))
        }
        "tcp_stream" | "tcpstream" => {
            let server = tcp_stream::TcpStreamServerConfig::parse(map, position)
                .context("failed to load this TcpStream server")?;
            Ok(AnyServerConfig::TcpStream(server))
        }
        #[cfg(any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "openbsd"
        ))]
        "tcp_tproxy" | "tcptproxy" => {
            let server = tcp_tproxy::TcpTProxyServerConfig::parse(map, position)
                .context("failed to load this TcpTProxy server")?;
            Ok(AnyServerConfig::TcpTProxy(server))
        }
        "tls_stream" | "tlsstream" => {
            let server = tls_stream::TlsStreamServerConfig::parse(map, position)
                .context("failed to load this TLsStream server")?;
            Ok(AnyServerConfig::TlsStream(server))
        }
        "sni_proxy" | "sniproxy" => {
            let server = sni_proxy::SniProxyServerConfig::parse(map, position)
                .context("failed to load this SniProxy server")?;
            Ok(AnyServerConfig::SniProxy(server))
        }
        "socks_proxy" | "socksproxy" => {
            let server = socks_proxy::SocksProxyServerConfig::parse(map, position)
                .context("failed to load this SocksProxy server")?;
            Ok(AnyServerConfig::SocksProxy(server))
        }
        "http_proxy" | "httpproxy" => {
            let server = http_proxy::HttpProxyServerConfig::parse(map, position)
                .context("failed to load this HttpProxy server")?;
            Ok(AnyServerConfig::HttpProxy(server))
        }
        "http_rproxy" | "httprproxy" | "http_reverse_proxy" | "httpreverseproxy"
        | "http_gateway" | "httpgateway" => {
            let server = http_rproxy::HttpRProxyServerConfig::parse(map, position)
                .context("failed to load this HttpRProxy server")?;
            Ok(AnyServerConfig::HttpRProxy(server))
        }
        _ => Err(anyhow!("unsupported server type {}", server_type)),
    }
}

fn build_topology_map() -> anyhow::Result<TopoMap> {
    let mut topo_map = TopoMap::default();

    for name in registry::get_all_names() {
        topo_map.add_node(&name, &|name| {
            let conf = registry::get(name)?;
            conf.dependent_server()
        })?;
    }

    Ok(topo_map)
}

pub(crate) fn get_all_sorted() -> anyhow::Result<Vec<Arc<AnyServerConfig>>> {
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
