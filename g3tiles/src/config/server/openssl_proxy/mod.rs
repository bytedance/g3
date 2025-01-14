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

use g3_io_ext::LimitedCopyConfig;
use g3_tls_ticket::TlsTicketConfig;
use g3_types::acl::AclNetworkRuleBuilder;
use g3_types::metrics::{NodeName, StaticMetricsTags};
use g3_types::net::{TcpListenConfig, TcpMiscSockOpts, TcpSockSpeedLimitConfig};
use g3_types::route::HostMatch;
use g3_yaml::YamlDocPosition;

use super::{ServerConfig, IDLE_CHECK_DEFAULT_DURATION, IDLE_CHECK_MAXIMUM_DURATION};
use crate::config::server::{AnyServerConfig, ServerConfigDiffAction};

mod host;
pub(crate) use host::OpensslHostConfig;

const SERVER_CONFIG_TYPE: &str = "OpensslProxy";

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OpensslProxyServerConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) shared_logger: Option<AsciiString>,
    pub(crate) listen: TcpListenConfig,
    pub(crate) listen_in_worker: bool,
    pub(crate) ingress_net_filter: Option<AclNetworkRuleBuilder>,
    pub(crate) extra_metrics_tags: Option<Arc<StaticMetricsTags>>,
    pub(crate) client_hello_recv_timeout: Duration,
    pub(crate) client_hello_max_size: u32,
    pub(crate) accept_timeout: Duration,
    pub(crate) hosts: HostMatch<Arc<OpensslHostConfig>>,
    pub(crate) tcp_sock_speed_limit: TcpSockSpeedLimitConfig,
    pub(crate) task_idle_check_duration: Duration,
    pub(crate) task_idle_max_count: i32,
    pub(crate) tcp_copy: LimitedCopyConfig,
    pub(crate) tcp_misc_opts: TcpMiscSockOpts,
    pub(crate) tls_ticketer: Option<TlsTicketConfig>,
    #[cfg(feature = "openssl-async-job")]
    pub(crate) tls_no_async_mode: bool,
    pub(crate) spawn_task_unconstrained: bool,
    pub(crate) alert_unrecognized_name: bool,
}

impl OpensslProxyServerConfig {
    pub(crate) fn new(position: Option<YamlDocPosition>) -> Self {
        OpensslProxyServerConfig {
            name: NodeName::default(),
            position,
            shared_logger: None,
            listen: TcpListenConfig::default(),
            listen_in_worker: false,
            ingress_net_filter: None,
            extra_metrics_tags: None,
            client_hello_recv_timeout: Duration::from_secs(10),
            client_hello_max_size: 16384, // 16K
            accept_timeout: Duration::from_secs(60),
            hosts: HostMatch::default(),
            tcp_sock_speed_limit: TcpSockSpeedLimitConfig::default(),
            task_idle_check_duration: IDLE_CHECK_DEFAULT_DURATION,
            task_idle_max_count: 1,
            tcp_copy: Default::default(),
            tcp_misc_opts: Default::default(),
            tls_ticketer: None,
            #[cfg(feature = "openssl-async-job")]
            tls_no_async_mode: false,
            spawn_task_unconstrained: false,
            alert_unrecognized_name: false,
        }
    }

    pub(super) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut server = OpensslProxyServerConfig::new(position);

        g3_yaml::foreach_kv(map, |k, v| server.set(k, v))?;

        server.check()?;
        Ok(server)
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.hosts.is_empty() {
            return Err(anyhow!("no host config set"));
        }
        if self.task_idle_check_duration > IDLE_CHECK_MAXIMUM_DURATION {
            self.task_idle_check_duration = IDLE_CHECK_MAXIMUM_DURATION;
        }
        Ok(())
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_SERVER_TYPE => Ok(()),
            super::CONFIG_KEY_SERVER_NAME => {
                self.name = g3_yaml::value::as_metrics_name(v)?;
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
            "client_hello_recv_timeout" => {
                self.client_hello_recv_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "client_hello_max_size" => {
                self.client_hello_max_size = g3_yaml::humanize::as_u32(v)
                    .context(format!("invalid humanize u32 value for key {k}"))?;
                Ok(())
            }
            "accept_timeout" | "handshake_timeout" | "negotiation_timeout" => {
                self.accept_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "virtual_hosts" | "hosts" => {
                self.hosts = g3_yaml::value::as_host_matched_obj(v, self.position.as_ref())?;
                Ok(())
            }
            "tcp_sock_speed_limit" | "tcp_conn_speed_limit" => {
                self.tcp_sock_speed_limit = g3_yaml::value::as_tcp_sock_speed_limit(v)
                    .context(format!("invalid tcp socket speed limit value for key {k}"))?;
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
            "tls_ticketer" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(self.position.as_ref())?;
                let ticketer = TlsTicketConfig::parse_yaml(v, Some(lookup_dir))
                    .context(format!("invalid tls ticket config value for key {k}"))?;
                self.tls_ticketer = Some(ticketer);
                Ok(())
            }
            #[cfg(feature = "openssl-async-job")]
            "tls_no_async_mode" => {
                self.tls_no_async_mode = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "spawn_task_unconstrained" | "task_unconstrained" => {
                self.spawn_task_unconstrained = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "alert_unrecognized_name" => {
                self.alert_unrecognized_name = g3_yaml::value::as_bool(v)
                    .context(format!("invalid bool value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}

impl ServerConfig for OpensslProxyServerConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn server_type(&self) -> &'static str {
        SERVER_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyServerConfig) -> ServerConfigDiffAction {
        let AnyServerConfig::OpensslProxy(new) = new else {
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
}
