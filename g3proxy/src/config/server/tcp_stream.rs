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
use g3_types::acl::AclNetworkRuleBuilder;
use g3_types::collection::SelectivePickPolicy;
use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_types::net::{
    OpensslTlsClientConfigBuilder, TcpListenConfig, TcpMiscSockOpts, TcpSockSpeedLimitConfig,
    WeightedUpstreamAddr,
};
use g3_yaml::YamlDocPosition;

use super::{AnyServerConfig, ServerConfig, ServerConfigDiffAction, IDLE_CHECK_MAXIMUM_DURATION};

const SERVER_CONFIG_TYPE: &str = "TcpStream";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TcpStreamServerConfig {
    name: MetricsName,
    position: Option<YamlDocPosition>,
    pub(crate) escaper: MetricsName,
    pub(crate) auditor: MetricsName,
    pub(crate) shared_logger: Option<AsciiString>,
    pub(crate) listen: TcpListenConfig,
    pub(crate) listen_in_worker: bool,
    pub(crate) client_tls_config: Option<OpensslTlsClientConfigBuilder>,
    pub(crate) ingress_net_filter: Option<AclNetworkRuleBuilder>,
    pub(crate) upstream: Vec<WeightedUpstreamAddr>,
    pub(crate) upstream_pick_policy: SelectivePickPolicy,
    pub(crate) upstream_tls_name: Option<String>,
    pub(crate) tcp_sock_speed_limit: TcpSockSpeedLimitConfig,
    pub(crate) task_idle_check_duration: Duration,
    pub(crate) task_idle_max_count: i32,
    pub(crate) tcp_copy: LimitedCopyConfig,
    pub(crate) tcp_misc_opts: TcpMiscSockOpts,
    pub(crate) extra_metrics_tags: Option<Arc<StaticMetricsTags>>,
}

impl TcpStreamServerConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        TcpStreamServerConfig {
            name: MetricsName::default(),
            position,
            escaper: MetricsName::default(),
            auditor: MetricsName::default(),
            shared_logger: None,
            listen: TcpListenConfig::default(),
            listen_in_worker: false,
            client_tls_config: None,
            ingress_net_filter: None,
            upstream: Vec::new(),
            upstream_pick_policy: SelectivePickPolicy::Random,
            upstream_tls_name: None,
            tcp_sock_speed_limit: TcpSockSpeedLimitConfig::default(),
            task_idle_check_duration: Duration::from_secs(300),
            task_idle_max_count: 1,
            tcp_copy: Default::default(),
            tcp_misc_opts: Default::default(),
            extra_metrics_tags: None,
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut server = TcpStreamServerConfig::new(position);

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
                self.listen = g3_yaml::value::as_tcp_listen_config(v)
                    .context(format!("invalid tcp listen config value for key {k}"))?;
                Ok(())
            }
            "listen_in_worker" => {
                self.listen_in_worker = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "tls_client" => {
                if let Yaml::Boolean(enable) = v {
                    if *enable {
                        self.client_tls_config =
                            Some(OpensslTlsClientConfigBuilder::with_cache_for_one_site());
                    }
                } else {
                    let lookup_dir = crate::config::get_lookup_dir(self.position.as_ref());
                    let builder = g3_yaml::value::as_to_one_openssl_tls_client_config_builder(
                        v,
                        Some(&lookup_dir),
                    )
                    .context(format!(
                        "invalid openssl tls client config value for key {k}"
                    ))?;
                    self.client_tls_config = Some(builder);
                }
                Ok(())
            }
            "ingress_network_filter" | "ingress_net_filter" => {
                let filter = g3_yaml::value::acl::as_ingress_network_rule_builder(v).context(
                    format!("invalid ingress network acl rule value for key {k}"),
                )?;
                self.ingress_net_filter = Some(filter);
                Ok(())
            }
            "upstream" | "proxy_pass" => {
                match v {
                    Yaml::String(_) => {
                        let node = g3_yaml::value::as_weighted_upstream_addr(v, 0)?;
                        self.upstream.push(node);
                    }
                    Yaml::Array(seq) => {
                        for (i, v) in seq.iter().enumerate() {
                            let node = g3_yaml::value::as_weighted_upstream_addr(v, 0)
                                .context(format!("invalid value for {k}#{i}"))?;
                            self.upstream.push(node);
                        }
                    }
                    _ => return Err(anyhow!("invalid value type for key {k}")),
                }
                Ok(())
            }
            "upstream_pick_policy" => {
                self.upstream_pick_policy = g3_yaml::value::as_selective_pick_policy(v)?;
                Ok(())
            }
            "upstream_tls_name" => {
                let tls_name = g3_yaml::value::as_string(v)
                    .context(format!("invalid tls server name value for key {k}"))?;
                self.upstream_tls_name = Some(tls_name);
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
        if self.upstream.is_empty() {
            return Err(anyhow!("upstream is not set"));
        }
        if self.task_idle_check_duration > IDLE_CHECK_MAXIMUM_DURATION {
            self.task_idle_check_duration = IDLE_CHECK_MAXIMUM_DURATION;
        }
        if self.client_tls_config.is_some() && self.upstream_tls_name.is_none() {
            if let Some(upstream) = self.upstream.get(0) {
                self.upstream_tls_name = Some(upstream.inner().host().to_string());
            }
        }

        Ok(())
    }
}

impl ServerConfig for TcpStreamServerConfig {
    fn name(&self) -> &MetricsName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn server_type(&self) -> &'static str {
        SERVER_CONFIG_TYPE
    }

    fn escaper(&self) -> &MetricsName {
        &self.escaper
    }

    fn user_group(&self) -> &MetricsName {
        Default::default()
    }

    fn auditor(&self) -> &MetricsName {
        &self.auditor
    }

    fn diff_action(&self, new: &AnyServerConfig) -> ServerConfigDiffAction {
        let new = match new {
            AnyServerConfig::TcpStream(config) => config,
            _ => return ServerConfigDiffAction::SpawnNew,
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
