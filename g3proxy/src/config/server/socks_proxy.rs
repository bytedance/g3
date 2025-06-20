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

use g3_io_ext::{LimitedUdpRelayConfig, StreamCopyConfig};
use g3_types::acl::{AclExactPortRule, AclNetworkRuleBuilder};
use g3_types::acl_set::AclDstHostRuleSetBuilder;
use g3_types::metrics::{MetricTagMap, NodeName};
use g3_types::net::{
    PortRange, SocketBufferConfig, TcpListenConfig, TcpMiscSockOpts, TcpSockSpeedLimitConfig,
    UdpMiscSockOpts, UdpSockSpeedLimitConfig,
};
use g3_yaml::YamlDocPosition;

use super::{
    AnyServerConfig, IDLE_CHECK_DEFAULT_DURATION, IDLE_CHECK_DEFAULT_MAX_COUNT,
    IDLE_CHECK_MAXIMUM_DURATION, ServerConfig, ServerConfigDiffAction,
};

const SERVER_CONFIG_TYPE: &str = "SocksProxy";

/// collection of timeout config
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SocksProxyServerTimeoutConfig {
    /// for all commands: negotiation task should finish before this timeout
    pub(crate) negotiation: Duration,
    /// only for udp associate: client must send first udp packet before this timeout
    pub(crate) udp_client_initial: Duration,
}

impl Default for SocksProxyServerTimeoutConfig {
    /// this set default timeout values
    fn default() -> Self {
        SocksProxyServerTimeoutConfig {
            negotiation: Duration::from_secs(4),
            udp_client_initial: Duration::from_secs(30),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SocksProxyServerConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) escaper: NodeName,
    pub(crate) auditor: NodeName,
    pub(crate) user_group: NodeName,
    pub(crate) shared_logger: Option<AsciiString>,
    pub(crate) listen: Option<TcpListenConfig>,
    pub(crate) listen_in_worker: bool,
    pub(crate) use_udp_associate: bool,
    pub(crate) udp_bind4: Vec<IpAddr>,
    pub(crate) udp_bind6: Vec<IpAddr>,
    pub(crate) udp_bind_port_range: Option<PortRange>,
    pub(crate) udp_socket_buffer: SocketBufferConfig,
    pub(crate) ingress_net_filter: Option<AclNetworkRuleBuilder>,
    pub(crate) dst_host_filter: Option<AclDstHostRuleSetBuilder>,
    pub(crate) dst_port_filter: Option<AclExactPortRule>,
    pub(crate) tcp_sock_speed_limit: TcpSockSpeedLimitConfig,
    pub(crate) udp_sock_speed_limit: UdpSockSpeedLimitConfig,
    pub(crate) timeout: SocksProxyServerTimeoutConfig,
    pub(crate) task_idle_check_duration: Duration,
    pub(crate) task_idle_max_count: usize,
    pub(crate) flush_task_log_on_created: bool,
    pub(crate) flush_task_log_on_connected: bool,
    pub(crate) task_log_flush_interval: Option<Duration>,
    pub(crate) tcp_copy: StreamCopyConfig,
    pub(crate) udp_relay: LimitedUdpRelayConfig,
    pub(crate) tcp_misc_opts: TcpMiscSockOpts,
    pub(crate) udp_misc_opts: UdpMiscSockOpts,
    pub(crate) transmute_udp_echo_ip: Option<FxHashMap<IpAddr, IpAddr>>,
    pub(crate) extra_metrics_tags: Option<Arc<MetricTagMap>>,
}

impl SocksProxyServerConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        SocksProxyServerConfig {
            name: NodeName::default(),
            position,
            escaper: NodeName::default(),
            auditor: NodeName::default(),
            user_group: NodeName::default(),
            shared_logger: None,
            listen: None,
            listen_in_worker: false,
            use_udp_associate: false,
            udp_bind4: Vec::new(),
            udp_bind6: Vec::new(),
            udp_bind_port_range: None,
            udp_socket_buffer: SocketBufferConfig::default(),
            ingress_net_filter: None,
            dst_host_filter: None,
            dst_port_filter: None,
            tcp_sock_speed_limit: TcpSockSpeedLimitConfig::default(),
            udp_sock_speed_limit: UdpSockSpeedLimitConfig::default(),
            timeout: SocksProxyServerTimeoutConfig::default(),
            task_idle_check_duration: IDLE_CHECK_DEFAULT_DURATION,
            task_idle_max_count: IDLE_CHECK_DEFAULT_MAX_COUNT,
            flush_task_log_on_created: false,
            flush_task_log_on_connected: false,
            task_log_flush_interval: None,
            tcp_copy: Default::default(),
            udp_relay: Default::default(),
            tcp_misc_opts: Default::default(),
            udp_misc_opts: Default::default(),
            transmute_udp_echo_ip: None,
            extra_metrics_tags: None,
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut server = Self::new(position);

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
            "escaper" => {
                self.escaper = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "auditor" => {
                self.auditor = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "user_group" => {
                self.user_group = g3_yaml::value::as_metric_node_name(v)?;
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
            "listen" => {
                let config = g3_yaml::value::as_tcp_listen_config(v)
                    .context(format!("invalid tcp listen config value for key {k}"))?;
                self.listen = Some(config);
                Ok(())
            }
            "listen_in_worker" => {
                self.listen_in_worker = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "use_udp_associate" | "enable_udp_associate" | "udp_associate_enabled" => {
                self.use_udp_associate = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "udp_bind_ipv4" => {
                self.udp_bind4 = g3_yaml::value::as_list(v, |v| {
                    let ip4 = g3_yaml::value::as_ipv4addr(v)?;
                    Ok(IpAddr::V4(ip4))
                })?;
                Ok(())
            }
            "udp_bind_ipv6" => {
                self.udp_bind6 = g3_yaml::value::as_list(v, |v| {
                    let ip6 = g3_yaml::value::as_ipv6addr(v)?;
                    Ok(IpAddr::V6(ip6))
                })?;
                Ok(())
            }
            "udp_bind_port_range" => {
                let range = g3_yaml::value::as_port_range(v)
                    .context(format!("invalid port range value for key {k}"))?;
                self.udp_bind_port_range = Some(range);
                Ok(())
            }
            "udp_socket_buffer" => {
                self.udp_socket_buffer = g3_yaml::value::as_socket_buffer_config(v)
                    .context(format!("invalid socket buffer config value for key {k}"))?;
                Ok(())
            }
            "ingress_network_filter" | "ingress_net_filter" => {
                let filter = g3_yaml::value::acl::as_ingress_network_rule_builder(v).context(
                    format!("invalid ingress network acl rule value for key {k}"),
                )?;
                self.ingress_net_filter = Some(filter);
                Ok(())
            }
            "dst_host_filter_set" => {
                let filter_set = g3_yaml::value::acl_set::as_dst_host_rule_set_builder(v)
                    .context(format!("invalid dst host acl rule set value for key {k}"))?;
                self.dst_host_filter = Some(filter_set);
                Ok(())
            }
            "dst_port_filter" => {
                let filter = g3_yaml::value::acl::as_exact_port_rule(v)
                    .context(format!("invalid dst port acl rule for key {k}"))?;
                self.dst_port_filter = Some(filter);
                Ok(())
            }
            "tcp_sock_speed_limit" => {
                self.tcp_sock_speed_limit = g3_yaml::value::as_tcp_sock_speed_limit(v)
                    .context(format!("invalid tcp socket speed limit value for key {k}"))?;
                Ok(())
            }
            "tcp_conn_speed_limit" | "tcp_conn_limit" | "conn_limit" => {
                warn!("deprecated config key '{k}', please use 'tcp_sock_speed_limit' instead");
                self.set("tcp_sock_speed_limit", v)
            }
            "udp_sock_speed_limit"
            | "udp_relay_speed_limit"
            | "udp_relay_limit"
            | "relay_limit" => {
                self.udp_sock_speed_limit = g3_yaml::value::as_udp_sock_speed_limit(v)
                    .context(format!("invalid udp socket speed limit value for key {k}"))?;
                Ok(())
            }
            "tcp_copy_buffer_size" => {
                let buffer_size = g3_yaml::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                self.tcp_copy.set_buffer_size(buffer_size);
                Ok(())
            }
            "tcp_copy_yield_size" => {
                let yield_size = g3_yaml::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                self.tcp_copy.set_yield_size(yield_size);
                Ok(())
            }
            "udp_relay_packet_size" => {
                let packet_size = g3_yaml::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                self.udp_relay.set_packet_size(packet_size);
                Ok(())
            }
            "udp_relay_yield_size" => {
                let yield_size = g3_yaml::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                self.udp_relay.set_yield_size(yield_size);
                Ok(())
            }
            "udp_relay_batch_size" => {
                let batch_size = g3_yaml::value::as_usize(v)?;
                self.udp_relay.set_batch_size(batch_size);
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
            "negotiation_timeout" => {
                self.timeout.negotiation = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "udp_client_initial_timeout" => {
                self.timeout.udp_client_initial = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "task_idle_check_duration" => {
                self.task_idle_check_duration = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "task_idle_max_count" => {
                self.task_idle_max_count = g3_yaml::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                Ok(())
            }
            "flush_task_log_on_created" => {
                self.flush_task_log_on_created = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "flush_task_log_on_connected" => {
                self.flush_task_log_on_connected = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "task_log_flush_interval" => {
                let interval = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                self.task_log_flush_interval = Some(interval);
                Ok(())
            }
            "transmute_udp_echo_ip" => {
                if let Yaml::Hash(_) = v {
                    let map = g3_yaml::value::as_hashmap(
                        v,
                        g3_yaml::value::as_ipaddr,
                        g3_yaml::value::as_ipaddr,
                    )?;
                    self.transmute_udp_echo_ip = Some(map.into_iter().collect::<FxHashMap<_, _>>());
                } else {
                    let enable = g3_yaml::value::as_bool(v)?;
                    if enable {
                        self.transmute_udp_echo_ip = Some(FxHashMap::default());
                    }
                }
                Ok(())
            }
            "auto_reply_local_ip_map" => {
                warn!("deprecated config key '{k}', please use 'transmute_udp_echo_ip' instead");
                self.set("transmute_udp_echo_ip", v)
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.escaper.is_empty() {
            return Err(anyhow!("escaper is not set"));
        }
        if self.task_idle_check_duration > IDLE_CHECK_MAXIMUM_DURATION {
            self.task_idle_check_duration = IDLE_CHECK_MAXIMUM_DURATION;
        }

        Ok(())
    }

    pub(crate) fn transmute_udp_echo_addr(&self, local_addr: SocketAddr) -> SocketAddr {
        if let Some(map) = &self.transmute_udp_echo_ip {
            let ip = if let Some(ip) = map.get(&local_addr.ip()) {
                *ip
            } else {
                match local_addr.ip() {
                    IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                    IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                }
            };
            return SocketAddr::new(ip, local_addr.port());
        }
        local_addr
    }
}

impl ServerConfig for SocksProxyServerConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn r#type(&self) -> &'static str {
        SERVER_CONFIG_TYPE
    }

    fn escaper(&self) -> &NodeName {
        &self.escaper
    }

    fn user_group(&self) -> &NodeName {
        &self.user_group
    }

    fn auditor(&self) -> &NodeName {
        &self.auditor
    }

    fn diff_action(&self, new: &AnyServerConfig) -> ServerConfigDiffAction {
        let AnyServerConfig::SocksProxy(new) = new else {
            return ServerConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            return ServerConfigDiffAction::NoAction;
        }

        if self.listen != new.listen {
            return ServerConfigDiffAction::ReloadAndRespawn;
        }

        ServerConfigDiffAction::ReloadNoRespawn
    }

    fn shared_logger(&self) -> Option<&str> {
        self.shared_logger.as_ref().map(|s| s.as_str())
    }

    fn task_log_flush_interval(&self) -> Option<Duration> {
        self.task_log_flush_interval
    }

    #[inline]
    fn limited_copy_config(&self) -> StreamCopyConfig {
        self.tcp_copy
    }

    #[inline]
    fn task_max_idle_count(&self) -> usize {
        self.task_idle_max_count
    }
}
