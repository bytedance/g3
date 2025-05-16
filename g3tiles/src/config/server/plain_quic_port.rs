/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeSet;

use anyhow::{Context, anyhow};
use bitflags::bitflags;
use yaml_rust::{Yaml, yaml};

use g3_tls_ticket::TlsTicketConfig;
use g3_types::acl::AclNetworkRuleBuilder;
use g3_types::metrics::NodeName;
use g3_types::net::{RustlsServerConfigBuilder, UdpListenConfig};
use g3_yaml::YamlDocPosition;

use super::ServerConfig;
use crate::config::server::{AnyServerConfig, ServerConfigDiffAction};

const SERVER_CONFIG_TYPE: &str = "PlainQuicPort";

bitflags! {
    pub(crate) struct PlainQuicPortUpdateFlags: u64 {
        const LISTEN = 0b0001;
        const QUINN = 0b0010;
        const NEXT_SERVER = 0b0100;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PlainQuicPortConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) listen: UdpListenConfig,
    pub(crate) listen_in_worker: bool,
    pub(crate) tls_server: RustlsServerConfigBuilder,
    pub(crate) tls_ticketer: Option<TlsTicketConfig>,
    pub(crate) ingress_net_filter: Option<AclNetworkRuleBuilder>,
    pub(crate) server: NodeName,
    pub(crate) offline_rebind_port: Option<u16>,
}

impl PlainQuicPortConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        PlainQuicPortConfig {
            name: NodeName::default(),
            position,
            listen: UdpListenConfig::default(),
            listen_in_worker: false,
            tls_server: RustlsServerConfigBuilder::empty(),
            tls_ticketer: None,
            ingress_net_filter: None,
            server: NodeName::default(),
            offline_rebind_port: None,
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut server = PlainQuicPortConfig::new(position);

        g3_yaml::foreach_kv(map, |k, v| server.set(k, v))?;

        server.check()?;
        Ok(server)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_SERVER_TYPE => Ok(()),
            super::CONFIG_KEY_SERVER_NAME => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "listen" => {
                self.listen = g3_yaml::value::as_udp_listen_config(v)
                    .context(format!("invalid udp listen config value for key {k}"))?;
                Ok(())
            }
            "listen_in_worker" => {
                self.listen_in_worker = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "offline_rebind_port" => {
                let port = g3_yaml::value::as_u16(v)?;
                self.offline_rebind_port = Some(port);
                Ok(())
            }
            "quic_server" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(self.position.as_ref())?;
                self.tls_server =
                    g3_yaml::value::as_rustls_server_config_builder(v, Some(lookup_dir))?;
                Ok(())
            }
            "tls_ticketer" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(self.position.as_ref())?;
                let ticketer = TlsTicketConfig::parse_yaml(v, Some(lookup_dir))
                    .context(format!("invalid tls ticket config value for key {k}"))?;
                self.tls_ticketer = Some(ticketer);
                Ok(())
            }
            "ingress_network_filter" | "ingress_net_filter" => {
                let filter = g3_yaml::value::acl::as_ingress_network_rule_builder(v).context(
                    format!("invalid ingress network acl rule value for key {k}"),
                )?;
                self.ingress_net_filter = Some(filter);
                Ok(())
            }
            "server" => {
                self.server = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.server.is_empty() {
            return Err(anyhow!("server is not set"));
        }
        // make sure listen is always set
        self.listen.check().context("invalid listen config")?;
        self.tls_server.check().context("invalid quic tls config")?;

        Ok(())
    }
}

impl ServerConfig for PlainQuicPortConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn r#type(&self) -> &'static str {
        SERVER_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyServerConfig) -> ServerConfigDiffAction {
        let AnyServerConfig::PlainQuicPort(new) = new else {
            return ServerConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            return ServerConfigDiffAction::NoAction;
        }

        if self.listen_in_worker != new.listen_in_worker {
            return ServerConfigDiffAction::ReloadAndRespawn;
        }

        let mut flags = PlainQuicPortUpdateFlags::empty();
        if self.listen != new.listen {
            flags.set(PlainQuicPortUpdateFlags::LISTEN, true);
        }
        if self.tls_server != new.tls_server {
            flags.set(PlainQuicPortUpdateFlags::QUINN, true);
        }
        if self.server != new.server {
            flags.set(PlainQuicPortUpdateFlags::NEXT_SERVER, true);
        }

        ServerConfigDiffAction::UpdateInPlace(flags.bits())
    }

    fn dependent_server(&self) -> Option<BTreeSet<NodeName>> {
        let mut set = BTreeSet::new();
        set.insert(self.server.clone());
        Some(set)
    }
}
