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

use anyhow::{anyhow, Context};
use yaml_rust::{yaml, Yaml};

use g3_types::acl::AclNetworkRuleBuilder;
use g3_types::metrics::MetricsName;
use g3_types::net::{RustlsServerConfigBuilder, TcpListenConfig};
use g3_yaml::YamlDocPosition;

use super::ServerConfig;
use crate::config::server::{AnyServerConfig, ServerConfigDiffAction};

const SERVER_CONFIG_TYPE: &str = "PlainTlsPort";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PlainTlsPortConfig {
    name: String,
    position: Option<YamlDocPosition>,
    pub(crate) listen: TcpListenConfig,
    pub(crate) listen_in_worker: bool,
    pub(crate) ingress_net_filter: Option<AclNetworkRuleBuilder>,
    pub(crate) server_tls_config: Option<RustlsServerConfigBuilder>,
    pub(crate) server: String,
}

impl PlainTlsPortConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        PlainTlsPortConfig {
            name: String::new(),
            position,
            listen: TcpListenConfig::default(),
            listen_in_worker: false,
            ingress_net_filter: None,
            server_tls_config: None,
            server: String::new(),
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut server = PlainTlsPortConfig::new(position);

        g3_yaml::foreach_kv(map, |k, v| server.set(k, v))?;

        server.check()?;
        Ok(server)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_SERVER_TYPE => Ok(()),
            super::CONFIG_KEY_SERVER_NAME => {
                if let Yaml::String(name) = v {
                    self.name.clone_from(name);
                    Ok(())
                } else {
                    Err(anyhow!("invalid string value for key {k}"))
                }
            }
            "listen" => {
                self.listen = g3_yaml::value::as_tcp_listen_config(v)
                    .context(format!("invalid tcp listen config value for key {k}"))?;
                Ok(())
            }
            "listen_in_worker" => {
                self.listen_in_worker = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "ingress_network_filter" | "ingress_net_filter" => {
                let filter = g3_yaml::value::acl::as_ingress_network_rule_builder(v).context(
                    format!("invalid ingress network acl rule value for key {k}"),
                )?;
                self.ingress_net_filter = Some(filter);
                Ok(())
            }
            "tls" | "tls_server" => {
                let lookup_dir = crate::config::get_lookup_dir(self.position.as_ref());
                let builder = g3_yaml::value::as_rustls_server_config_builder(v, Some(&lookup_dir))
                    .context(format!("invalid server tls config value for key {k}"))?;
                self.server_tls_config = Some(builder);
                Ok(())
            }
            "server" => {
                if let Yaml::String(name) = v {
                    self.server.clone_from(name);
                    Ok(())
                } else {
                    Err(anyhow!("invalid string value for key {k}"))
                }
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
        if self.server_tls_config.is_none() {
            return Err(anyhow!("tls server config is not set"));
        }

        Ok(())
    }
}

impl ServerConfig for PlainTlsPortConfig {
    fn name(&self) -> &str {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn server_type(&self) -> &'static str {
        SERVER_CONFIG_TYPE
    }

    fn escaper(&self) -> &str {
        ""
    }

    fn user_group(&self) -> &MetricsName {
        Default::default()
    }

    fn auditor(&self) -> &MetricsName {
        Default::default()
    }

    fn diff_action(&self, new: &AnyServerConfig) -> ServerConfigDiffAction {
        let new = match new {
            AnyServerConfig::PlainTlsPort(config) => config,
            _ => return ServerConfigDiffAction::SpawnNew,
        };

        if self.eq(new) {
            return ServerConfigDiffAction::NoAction;
        }

        if self.listen != new.listen {
            return ServerConfigDiffAction::ReloadAndRespawn;
        }

        ServerConfigDiffAction::UpdateInPlace(0)
    }

    fn dependent_server(&self) -> Option<BTreeSet<String>> {
        let mut set = BTreeSet::new();
        set.insert(self.server.clone());
        Some(set)
    }
}
