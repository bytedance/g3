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

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use ascii::AsciiString;
use yaml_rust::{yaml, Yaml};

use g3_dpi::{ProtocolInspectionConfig, ProtocolPortMap};
use g3_io_ext::LimitedCopyConfig;
use g3_types::acl::AclNetworkRuleBuilder;
use g3_types::metrics::{NodeName, StaticMetricsTags};
use g3_types::net::{TcpListenConfig, TcpMiscSockOpts, TcpSockSpeedLimitConfig};
use g3_types::route::HostMatch;
use g3_yaml::YamlDocPosition;

use super::{AnyServerConfig, ServerConfig, ServerConfigDiffAction, IDLE_CHECK_MAXIMUM_DURATION};

mod host;
pub(crate) use host::SniHostConfig;

const SERVER_CONFIG_TYPE: &str = "SniProxy";

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SniProxyServerConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) escaper: NodeName,
    pub(crate) auditor: NodeName,
    pub(crate) shared_logger: Option<AsciiString>,
    pub(crate) listen: Option<TcpListenConfig>,
    pub(crate) listen_in_worker: bool,
    pub(crate) ingress_net_filter: Option<AclNetworkRuleBuilder>,
    pub(crate) tcp_sock_speed_limit: TcpSockSpeedLimitConfig,
    pub(crate) task_idle_check_duration: Duration,
    pub(crate) task_idle_max_count: i32,
    pub(crate) flush_task_log_on_created: bool,
    pub(crate) flush_task_log_on_connected: bool,
    pub(crate) task_log_flush_interval: Option<Duration>,
    pub(crate) tcp_copy: LimitedCopyConfig,
    pub(crate) tcp_misc_opts: TcpMiscSockOpts,
    pub(crate) tls_max_client_hello_size: u32,
    pub(crate) request_wait_timeout: Duration,
    pub(crate) request_recv_timeout: Duration,
    pub(crate) protocol_inspection: ProtocolInspectionConfig,
    pub(crate) server_tcp_portmap: ProtocolPortMap,
    pub(crate) client_tcp_portmap: ProtocolPortMap,
    pub(crate) extra_metrics_tags: Option<Arc<StaticMetricsTags>>,
    pub(crate) allowed_sites: Option<HostMatch<Arc<SniHostConfig>>>,
}

impl SniProxyServerConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        SniProxyServerConfig {
            name: NodeName::default(),
            position,
            escaper: NodeName::default(),
            auditor: NodeName::default(),
            shared_logger: None,
            listen: None,
            listen_in_worker: false,
            ingress_net_filter: None,
            tcp_sock_speed_limit: TcpSockSpeedLimitConfig::default(),
            task_idle_check_duration: Duration::from_secs(300),
            task_idle_max_count: 1,
            flush_task_log_on_created: false,
            flush_task_log_on_connected: false,
            task_log_flush_interval: None,
            tcp_copy: Default::default(),
            tcp_misc_opts: Default::default(),
            tls_max_client_hello_size: 1 << 16,
            request_wait_timeout: Duration::from_secs(60),
            request_recv_timeout: Duration::from_secs(4),
            protocol_inspection: ProtocolInspectionConfig::default(),
            server_tcp_portmap: ProtocolPortMap::tcp_server(),
            client_tcp_portmap: ProtocolPortMap::tcp_client(),
            extra_metrics_tags: None,
            allowed_sites: None,
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut server = SniProxyServerConfig::new(position);

        g3_yaml::foreach_kv(map, |k, v| server.set(k, v))?;

        server.check()?;
        Ok(server)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_SERVER_TYPE => Ok(()),
            super::CONFIG_KEY_SERVER_NAME => {
                self.name = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "escaper" => {
                self.escaper = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "auditor" => {
                self.auditor = g3_yaml::value::as_metrics_name(v)?;
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
            "ingress_network_filter" | "ingress_net_filter" => {
                let filter = g3_yaml::value::acl::as_ingress_network_rule_builder(v).context(
                    format!("invalid ingress network acl rule value for key {k}"),
                )?;
                self.ingress_net_filter = Some(filter);
                Ok(())
            }
            "tcp_sock_speed_limit" | "tcp_conn_speed_limit" | "tcp_conn_limit" | "conn_limit" => {
                self.tcp_sock_speed_limit = g3_yaml::value::as_tcp_sock_speed_limit(v)
                    .context(format!("invalid tcp socket speed limit value for key {k}"))?;
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
            "tcp_misc_opts" => {
                self.tcp_misc_opts = g3_yaml::value::as_tcp_misc_sock_opts(v)
                    .context(format!("invalid tcp misc sock opts value for key {k}"))?;
                Ok(())
            }
            "tls_max_client_hello_size" => {
                self.tls_max_client_hello_size = g3_yaml::value::as_u32(v)?;
                Ok(())
            }
            "task_idle_check_duration" => {
                self.task_idle_check_duration = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "task_idle_max_count" => {
                self.task_idle_max_count =
                    g3_yaml::value::as_i32(v).context(format!("invalid i32 value for key {k}"))?;
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
            "request_wait_timeout" => {
                self.request_wait_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "request_recv_timeout" => {
                self.request_recv_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "protocol_inspection" => {
                let protocol_inspection = g3_yaml::value::as_protocol_inspection_config(v)
                    .context(format!(
                        "invalid protocol inspection config value for key {k}"
                    ))?;
                self.protocol_inspection = protocol_inspection;
                Ok(())
            }
            "server_tcp_portmap" => {
                g3_yaml::value::update_protocol_portmap(&mut self.server_tcp_portmap, v)
                    .context(format!("invalid protocol portmap value for key {k}"))
            }
            "client_tcp_portmap" => {
                g3_yaml::value::update_protocol_portmap(&mut self.client_tcp_portmap, v)
                    .context(format!("invalid protocol portmap value for key {k}"))
            }
            "allowed_hosts" | "allowed_sites" => {
                let allowed_sites = g3_yaml::value::as_host_matched_obj(v, self.position.as_ref())
                    .context(format!(
                        "invalid host matched SniHostConfig value for key {k}"
                    ))?;
                self.allowed_sites = Some(allowed_sites);
                Ok(())
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
}

impl ServerConfig for SniProxyServerConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn server_type(&self) -> &'static str {
        SERVER_CONFIG_TYPE
    }

    fn escaper(&self) -> &NodeName {
        &self.escaper
    }

    fn user_group(&self) -> &NodeName {
        Default::default()
    }

    fn auditor(&self) -> &NodeName {
        &self.auditor
    }

    fn diff_action(&self, new: &AnyServerConfig) -> ServerConfigDiffAction {
        let AnyServerConfig::SniProxy(new) = new else {
            return ServerConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            return ServerConfigDiffAction::NoAction;
        }

        if self.listen != new.listen {
            return ServerConfigDiffAction::ReloadAndRespawn;
        }

        ServerConfigDiffAction::ReloadOnlyConfig
    }

    fn shared_logger(&self) -> Option<&str> {
        self.shared_logger.as_ref().map(|s| s.as_str())
    }

    #[inline]
    fn limited_copy_config(&self) -> LimitedCopyConfig {
        self.tcp_copy
    }
    #[inline]
    fn task_idle_check_duration(&self) -> Duration {
        self.task_idle_check_duration
    }
    #[inline]
    fn task_max_idle_count(&self) -> i32 {
        self.task_idle_max_count
    }
}
