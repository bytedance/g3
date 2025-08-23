/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_types::collection::SelectivePickPolicy;
use g3_types::metrics::NodeName;
use g3_types::net::SocketBufferConfig;
use g3_yaml::YamlDocPosition;

use super::{AnyEscaperConfig, EscaperConfig, EscaperConfigDiffAction};

const ESCAPER_CONFIG_TYPE: &str = "RouteQuery";

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct RouteQueryEscaperConfig {
    pub(crate) name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) query_pass_client_ip: bool,
    pub(crate) query_allowed_nodes: BTreeSet<NodeName>,
    pub(crate) fallback_node: NodeName,
    pub(crate) cache_request_batch_count: usize,
    pub(crate) cache_request_timeout: Duration,
    pub(crate) cache_pick_policy: SelectivePickPolicy,
    pub(crate) query_peer_addr: SocketAddr,
    pub(crate) query_socket_buffer: SocketBufferConfig,
    pub(crate) query_wait_timeout: Duration,
    pub(crate) protective_cache_ttl: u32,
    pub(crate) maximum_cache_ttl: u32,
    pub(crate) cache_vanish_wait: Duration,
}

impl RouteQueryEscaperConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        RouteQueryEscaperConfig {
            name: NodeName::default(),
            position,
            query_pass_client_ip: false,
            query_allowed_nodes: BTreeSet::new(),
            fallback_node: NodeName::default(),
            cache_request_batch_count: 10,
            cache_request_timeout: Duration::from_millis(100),
            cache_pick_policy: SelectivePickPolicy::Ketama,
            query_peer_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1053),
            query_socket_buffer: SocketBufferConfig::default(),
            query_wait_timeout: Duration::from_secs(10),
            protective_cache_ttl: 10,
            maximum_cache_ttl: 1800,
            cache_vanish_wait: Duration::from_secs(30),
        }
    }

    pub(super) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut config = Self::new(position);

        g3_yaml::foreach_kv(map, |k, v| config.set(k, v))?;

        config.check()?;
        Ok(config)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_ESCAPER_TYPE => Ok(()),
            super::CONFIG_KEY_ESCAPER_NAME => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "query_pass_client_ip" => {
                self.query_pass_client_ip = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "query_allowed_next" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let name = g3_yaml::value::as_metric_node_name(v)
                            .context(format!("invalid metrics name value for {k}#{i}"))?;
                        // duplicate values should report an error
                        if let Some(old) = self.query_allowed_nodes.replace(name) {
                            return Err(anyhow!("found duplicate next node: {old}"));
                        }
                    }
                    Ok(())
                } else {
                    Err(anyhow!("invalid array value for key {k}"))
                }
            }
            "fallback_node" => {
                self.fallback_node = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "cache_request_batch_count" => {
                self.cache_request_batch_count = g3_yaml::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                Ok(())
            }
            "cache_request_timeout" => {
                self.cache_request_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "cache_pick_policy" => {
                self.cache_pick_policy = g3_yaml::value::as_selective_pick_policy(v)
                    .context(format!("invalid selective pick policy value for key {k}"))?;
                Ok(())
            }
            "query_peer_addr" | "query_peer_address" => {
                self.query_peer_addr = g3_yaml::value::as_env_sockaddr(v).context(format!(
                    "invalid query peer socket address value for key {k}"
                ))?;
                Ok(())
            }
            "query_socket_buffer" => {
                self.query_socket_buffer = g3_yaml::value::as_socket_buffer_config(v)
                    .context(format!("invalid socket buffer config value for key {k}"))?;
                Ok(())
            }
            "query_wait_timeout" => {
                self.query_wait_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "protective_cache_ttl" => {
                self.protective_cache_ttl =
                    g3_yaml::value::as_u32(v).context(format!("invalid u32 value for key {k}"))?;
                Ok(())
            }
            "maximum_cache_ttl" => {
                self.maximum_cache_ttl =
                    g3_yaml::value::as_u32(v).context(format!("invalid u32 value for key {k}"))?;
                Ok(())
            }
            "cache_vanish_wait" | "vanish_after_expired" => {
                self.cache_vanish_wait = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.query_allowed_nodes.is_empty() {
            return Err(anyhow!("no query allowed escapers found"));
        }
        if self.fallback_node.is_empty() {
            return Err(anyhow!("no fallback escaper found"));
        }

        Ok(())
    }
}

impl EscaperConfig for RouteQueryEscaperConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn r#type(&self) -> &str {
        ESCAPER_CONFIG_TYPE
    }

    fn resolver(&self) -> &NodeName {
        Default::default()
    }

    fn diff_action(&self, new: &AnyEscaperConfig) -> EscaperConfigDiffAction {
        let AnyEscaperConfig::RouteQuery(new) = new else {
            return EscaperConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            return EscaperConfigDiffAction::NoAction;
        }

        EscaperConfigDiffAction::Reload
    }

    fn dependent_escaper(&self) -> Option<BTreeSet<NodeName>> {
        let mut set = BTreeSet::new();
        for name in &self.query_allowed_nodes {
            set.insert(name.clone());
        }
        set.insert(self.fallback_node.clone());
        Some(set)
    }
}
