/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;

use anyhow::{Context, anyhow};
use ascii::AsciiString;
use yaml_rust::{Yaml, yaml};

use g3_types::collection::SelectivePickPolicy;
use g3_types::metrics::{MetricTagMap, NodeName};
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "illumos",
    target_os = "solaris"
))]
use g3_types::net::Interface;
use g3_types::net::{
    HappyEyeballsConfig, Host, TcpKeepAliveConfig, TcpMiscSockOpts, WeightedUpstreamAddr,
};
use g3_types::resolve::{QueryStrategy, ResolveStrategy};
use g3_yaml::YamlDocPosition;

use super::{AnyEscaperConfig, EscaperConfig, EscaperConfigDiffAction, GeneralEscaperConfig};

const ESCAPER_CONFIG_TYPE: &str = "DivertTcp";

#[derive(Clone, PartialEq)]
pub(crate) struct DivertTcpEscaperConfig {
    pub(crate) name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) shared_logger: Option<AsciiString>,
    pub(crate) proxy_nodes: Vec<WeightedUpstreamAddr>,
    pub(crate) proxy_pick_policy: SelectivePickPolicy,
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "illumos",
        target_os = "solaris"
    ))]
    pub(crate) bind_interface: Option<Interface>,
    pub(crate) bind_v4: Option<Ipv4Addr>,
    pub(crate) bind_v6: Option<Ipv6Addr>,
    pub(crate) no_ipv4: bool,
    pub(crate) no_ipv6: bool,
    pub(crate) resolver: NodeName,
    pub(crate) resolve_strategy: ResolveStrategy,
    pub(crate) general: GeneralEscaperConfig,
    pub(crate) happy_eyeballs: HappyEyeballsConfig,
    pub(crate) tcp_keepalive: TcpKeepAliveConfig,
    pub(crate) tcp_misc_opts: TcpMiscSockOpts,
    pub(crate) extra_metrics_tags: Option<Arc<MetricTagMap>>,
}

impl DivertTcpEscaperConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        DivertTcpEscaperConfig {
            name: NodeName::default(),
            position,
            shared_logger: None,
            proxy_nodes: Vec::with_capacity(1),
            proxy_pick_policy: SelectivePickPolicy::Random,
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "macos",
                target_os = "illumos",
                target_os = "solaris"
            ))]
            bind_interface: None,
            bind_v4: None,
            bind_v6: None,
            no_ipv4: false,
            no_ipv6: false,
            resolver: NodeName::default(),
            resolve_strategy: Default::default(),
            general: Default::default(),
            happy_eyeballs: Default::default(),
            tcp_keepalive: Default::default(),
            tcp_misc_opts: Default::default(),
            extra_metrics_tags: None,
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
            "shared_logger" => {
                let name = g3_yaml::value::as_ascii(v)?;
                self.shared_logger = Some(name);
                Ok(())
            }
            "extra_metrics_tags" => {
                let tags = g3_yaml::value::as_static_metrics_tags(v)
                    .context(format!("invalid static metrics tags value for key {k}"))?;
                self.extra_metrics_tags = Some(Arc::new(tags));
                Ok(())
            }
            "proxy_addr" => {
                self.proxy_nodes = g3_yaml::value::as_list(v, |v| {
                    g3_yaml::value::as_weighted_upstream_addr(v, 3128)
                })
                .context(format!(
                    "invalid weighted upstream address list value for key {k}"
                ))?;
                Ok(())
            }
            "proxy_addr_pick_policy" => {
                self.proxy_pick_policy = g3_yaml::value::as_selective_pick_policy(v)?;
                Ok(())
            }
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "macos",
                target_os = "illumos",
                target_os = "solaris"
            ))]
            "bind_interface" => {
                let interface = g3_yaml::value::as_interface(v)
                    .context(format!("invalid interface name value for key {k}"))?;
                self.bind_interface = Some(interface);
                Ok(())
            }
            "bind_ipv4" => {
                let ip4 = g3_yaml::value::as_ipv4addr(v)?;
                self.bind_v4 = Some(ip4);
                Ok(())
            }
            "bind_ipv6" => {
                let ip6 = g3_yaml::value::as_ipv6addr(v)?;
                self.bind_v6 = Some(ip6);
                Ok(())
            }
            "resolver" => {
                self.resolver = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "resolve_strategy" => {
                self.resolve_strategy = g3_yaml::value::as_resolve_strategy(v)?;
                Ok(())
            }
            "tcp_sock_speed_limit" | "tcp_conn_speed_limit" | "tcp_conn_limit" | "conn_limit" => {
                self.general.tcp_sock_speed_limit = g3_yaml::value::as_tcp_sock_speed_limit(v)
                    .context(format!("invalid tcp socket speed limit value for key {k}"))?;
                Ok(())
            }
            "tcp_keepalive" => {
                self.tcp_keepalive = g3_yaml::value::as_tcp_keepalive_config(v)
                    .context(format!("invalid tcp keepalive config value for key {k}"))?;
                Ok(())
            }
            "tcp_misc_opts" => {
                self.tcp_misc_opts = g3_yaml::value::as_tcp_misc_sock_opts(v)
                    .context(format!("invalid tcp misc sock opts value for key {k}"))?;
                Ok(())
            }
            "no_ipv4" => {
                self.no_ipv4 = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "no_ipv6" => {
                self.no_ipv6 = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "tcp_connect" => {
                self.general.tcp_connect = g3_yaml::value::as_tcp_connect_config(v)
                    .context(format!("invalid tcp connect value for key {k}"))?;
                Ok(())
            }
            "happy_eyeballs" => {
                self.happy_eyeballs = g3_yaml::value::as_happy_eyeballs_config(v)
                    .context(format!("invalid happy eyeballs config value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.proxy_nodes.is_empty() {
            return Err(anyhow!("proxy addr is not set"));
        }
        self.proxy_nodes.reverse(); // reverse as we push to the back
        if self.no_ipv4 && self.no_ipv6 {
            return Err(anyhow!("both ipv4 and ipv6 are disabled"));
        }

        let mut disable_ipv4 = true;
        let mut disable_ipv6 = true;
        let mut check_resolver = false;
        for node in &self.proxy_nodes {
            match node.inner().host() {
                Host::Domain(_) => {
                    disable_ipv4 = false;
                    disable_ipv6 = false;
                    check_resolver = true;
                }
                Host::Ip(IpAddr::V4(_)) => {
                    if self.no_ipv4 {
                        return Err(anyhow!("ipv4 is disable but the proxy addr is also ipv4"));
                    }
                    disable_ipv4 = false;
                }
                Host::Ip(IpAddr::V6(_)) => {
                    if self.no_ipv6 {
                        return Err(anyhow!("ipv6 is disable but the proxy addr is also ipv6"));
                    }
                    disable_ipv6 = false;
                }
            }
        }
        if disable_ipv4 {
            self.no_ipv4 = true;
        }
        if disable_ipv6 {
            self.no_ipv6 = true;
        }
        if check_resolver {
            if self.resolver.is_empty() {
                return Err(anyhow!("resolver is not set"));
            }
            self.resolve_strategy
                .update_query_strategy(self.no_ipv4, self.no_ipv6)
                .context("found incompatible resolver strategy")?;
            if !self.no_ipv4 && !self.no_ipv6 {
                match self.resolve_strategy.query {
                    QueryStrategy::Ipv4Only => self.no_ipv6 = true,
                    QueryStrategy::Ipv6Only => self.no_ipv4 = true,
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

impl EscaperConfig for DivertTcpEscaperConfig {
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
        &self.resolver
    }

    fn diff_action(&self, new: &AnyEscaperConfig) -> EscaperConfigDiffAction {
        let AnyEscaperConfig::DivertTcp(new) = new else {
            return EscaperConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            return EscaperConfigDiffAction::NoAction;
        }

        EscaperConfigDiffAction::Reload
    }

    fn shared_logger(&self) -> Option<&str> {
        self.shared_logger.as_ref().map(|s| s.as_str())
    }
}
