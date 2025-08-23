/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow};
use ascii::AsciiString;
use log::warn;
use rustc_hash::FxHashMap;
use yaml_rust::{Yaml, yaml};

use g3_types::auth::{Password, Username};
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
    HappyEyeballsConfig, Host, OpensslClientConfigBuilder, SocksAuth, TcpKeepAliveConfig,
    TcpMiscSockOpts, UdpMiscSockOpts, WeightedUpstreamAddr,
};
use g3_types::resolve::{QueryStrategy, ResolveStrategy};
use g3_yaml::YamlDocPosition;

use super::{AnyEscaperConfig, EscaperConfig, EscaperConfigDiffAction, GeneralEscaperConfig};

const ESCAPER_CONFIG_TYPE: &str = "ProxySocks5s";

#[derive(Clone, PartialEq)]
pub(crate) struct ProxySocks5sEscaperConfig {
    pub(crate) name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) shared_logger: Option<AsciiString>,
    pub(crate) proxy_nodes: Vec<WeightedUpstreamAddr>,
    pub(crate) proxy_pick_policy: SelectivePickPolicy,
    proxy_username: Username,
    proxy_password: Password,
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
    pub(crate) tls_config: OpensslClientConfigBuilder,
    pub(crate) tls_name: Option<Host>,
    pub(crate) resolver: NodeName,
    pub(crate) resolve_strategy: ResolveStrategy,
    pub(crate) general: GeneralEscaperConfig,
    pub(crate) happy_eyeballs: HappyEyeballsConfig,
    pub(crate) tcp_keepalive: TcpKeepAliveConfig,
    pub(crate) tcp_misc_opts: TcpMiscSockOpts,
    pub(crate) udp_misc_opts: UdpMiscSockOpts,
    pub(crate) auth_info: SocksAuth,
    pub(crate) peer_negotiation_timeout: Duration,
    transmute_udp_peer_ip: Option<FxHashMap<IpAddr, IpAddr>>,
    pub(crate) end_on_control_closed: bool,
    pub(crate) extra_metrics_tags: Option<Arc<MetricTagMap>>,
}

impl ProxySocks5sEscaperConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        ProxySocks5sEscaperConfig {
            name: NodeName::default(),
            position,
            shared_logger: None,
            proxy_nodes: Vec::with_capacity(1),
            proxy_pick_policy: SelectivePickPolicy::Random,
            proxy_username: Username::empty(),
            proxy_password: Password::empty(),
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
            tls_config: OpensslClientConfigBuilder::with_cache_for_many_sites(),
            tls_name: None,
            resolver: NodeName::default(),
            resolve_strategy: Default::default(),
            general: Default::default(),
            happy_eyeballs: Default::default(),
            tcp_keepalive: TcpKeepAliveConfig::default_enabled(),
            tcp_misc_opts: Default::default(),
            udp_misc_opts: Default::default(),
            auth_info: SocksAuth::None,
            peer_negotiation_timeout: Duration::from_secs(10),
            transmute_udp_peer_ip: None,
            end_on_control_closed: false,
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
                    g3_yaml::value::as_weighted_upstream_addr(v, 1080)
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
            "proxy_username" | "proxy_user" => {
                self.proxy_username = g3_yaml::value::as_username(v)
                    .context(format!("invalid username value for key {k}"))?;
                Ok(())
            }
            "proxy_password" | "proxy_passwd" => {
                self.proxy_password = g3_yaml::value::as_password(v)
                    .context(format!("invalid password value for key {k}"))?;
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
            "tcp_sock_speed_limit" => {
                self.general.tcp_sock_speed_limit = g3_yaml::value::as_tcp_sock_speed_limit(v)
                    .context(format!("invalid tcp socket speed limit value for key {k}"))?;
                Ok(())
            }
            "tcp_conn_speed_limit" | "tcp_conn_limit" | "conn_limit" => {
                warn!("deprecated config key '{k}', please use 'tcp_sock_speed_limit' instead");
                self.set("tcp_sock_speed_limit", v)
            }
            "udp_sock_speed_limit" => {
                self.general.udp_sock_speed_limit = g3_yaml::value::as_udp_sock_speed_limit(v)
                    .context(format!("invalid udp socket speed limit value for key {k}"))?;
                Ok(())
            }
            "udp_relay_speed_limit" | "udp_relay_limit" | "relay_limit" => {
                warn!("deprecated config key '{k}', please use 'udp_sock_speed_limit' instead");
                self.set("udp_sock_speed_limit", v)
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
            "udp_misc_opts" => {
                self.udp_misc_opts = g3_yaml::value::as_udp_misc_sock_opts(v)
                    .context(format!("invalid udp misc sock opts value for key {k}"))?;
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
            "tls" | "tls_client" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(self.position.as_ref())?;
                self.tls_config = g3_yaml::value::as_to_many_openssl_tls_client_config_builder(
                    v,
                    Some(lookup_dir),
                )
                .context(format!(
                    "invalid openssl tls client config value for key {k}"
                ))?;
                Ok(())
            }
            "tls_name" => {
                let name = g3_yaml::value::as_host(v)
                    .context(format!("invalid tls server name value for key {k}"))?;
                self.tls_name = Some(name);
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
            "peer_negotiation_timeout" => {
                self.peer_negotiation_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "transmute_udp_peer_ip" => {
                if let Yaml::Hash(_) = v {
                    let map = g3_yaml::value::as_hashmap(
                        v,
                        g3_yaml::value::as_ipaddr,
                        g3_yaml::value::as_ipaddr,
                    )
                    .context(format!("invalid IP:IP hashmap value for key {k}"))?;
                    self.transmute_udp_peer_ip = Some(map.into_iter().collect::<FxHashMap<_, _>>());
                } else {
                    let enable = g3_yaml::value::as_bool(v)?;
                    if enable {
                        self.transmute_udp_peer_ip = Some(FxHashMap::default());
                    }
                }
                Ok(())
            }
            "end_on_control_closed" => {
                self.end_on_control_closed = g3_yaml::value::as_bool(v)?;
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

        if !self.proxy_username.is_empty() {
            self.auth_info =
                SocksAuth::User(self.proxy_username.clone(), self.proxy_password.clone());
        }

        Ok(())
    }

    pub(crate) fn transmute_udp_peer_addr(
        &self,
        returned_addr: SocketAddr,
        tcp_peer_ip: IpAddr,
    ) -> SocketAddr {
        if let Some(map) = &self.transmute_udp_peer_ip {
            let ip = map.get(&returned_addr.ip()).copied().unwrap_or(tcp_peer_ip);
            SocketAddr::new(ip, returned_addr.port())
        } else if returned_addr.ip().is_unspecified() {
            SocketAddr::new(tcp_peer_ip, returned_addr.port())
        } else {
            returned_addr
        }
    }
}

impl EscaperConfig for ProxySocks5sEscaperConfig {
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
        let AnyEscaperConfig::ProxySocks5s(new) = new else {
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
