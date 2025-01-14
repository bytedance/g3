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
use std::time::Duration;

use anyhow::{anyhow, Context};
use slog::Logger;
use yaml_rust::{yaml, Yaml};

use g3_daemon::config::TopoMap;
use g3_types::metrics::NodeName;
use g3_yaml::{HybridParser, YamlDocPosition};

pub(crate) mod dummy_close;
#[cfg(feature = "quic")]
pub(crate) mod plain_quic_port;
pub(crate) mod plain_tcp_port;

pub(crate) mod keyless_proxy;
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
    fn name(&self) -> &NodeName;
    fn position(&self) -> Option<YamlDocPosition>;
    fn server_type(&self) -> &'static str;

    fn diff_action(&self, new: &AnyServerConfig) -> ServerConfigDiffAction;

    fn dependent_server(&self) -> Option<BTreeSet<NodeName>> {
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
    #[cfg(feature = "quic")]
    PlainQuicPort(Box<plain_quic_port::PlainQuicPortConfig>),
    OpensslProxy(openssl_proxy::OpensslProxyServerConfig),
    RustlsProxy(rustls_proxy::RustlsProxyServerConfig),
    KeylessProxy(keyless_proxy::KeylessProxyServerConfig),
}

macro_rules! impl_transparent0 {
    ($f:tt, $v:ty) => {
        pub(crate) fn $f(&self) -> $v {
            match self {
                AnyServerConfig::DummyClose(s) => s.$f(),
                AnyServerConfig::PlainTcpPort(s) => s.$f(),
                #[cfg(feature = "quic")]
                AnyServerConfig::PlainQuicPort(s) => s.$f(),
                AnyServerConfig::OpensslProxy(s) => s.$f(),
                AnyServerConfig::RustlsProxy(s) => s.$f(),
                AnyServerConfig::KeylessProxy(s) => s.$f(),
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
                #[cfg(feature = "quic")]
                AnyServerConfig::PlainQuicPort(s) => s.$f(p),
                AnyServerConfig::OpensslProxy(s) => s.$f(p),
                AnyServerConfig::RustlsProxy(s) => s.$f(p),
                AnyServerConfig::KeylessProxy(s) => s.$f(p),
            }
        }
    };
}

impl AnyServerConfig {
    impl_transparent0!(name, &NodeName);
    impl_transparent0!(position, Option<YamlDocPosition>);
    impl_transparent0!(server_type, &'static str);
    impl_transparent0!(dependent_server, Option<BTreeSet<NodeName>>);

    impl_transparent1!(diff_action, ServerConfigDiffAction, &Self);
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
        #[cfg(feature = "quic")]
        "plain_quic_port" | "plainquicport" | "plain_quic" | "plainquic" => {
            let server = plain_quic_port::PlainQuicPortConfig::parse(map, position)
                .context("failed to load this PlainQuicPort server")?;
            Ok(AnyServerConfig::PlainQuicPort(Box::new(server)))
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
        "keyless_proxy" | "keylessproxy" => {
            let server = keyless_proxy::KeylessProxyServerConfig::parse(map, position)
                .context("failed to load this KeylessProxy server")?;
            Ok(AnyServerConfig::KeylessProxy(server))
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
