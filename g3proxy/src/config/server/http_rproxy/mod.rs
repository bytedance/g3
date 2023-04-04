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
use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_types::net::{
    HttpForwardedHeaderType, HttpKeepAliveConfig, HttpServerId, RustlsServerConfigBuilder,
    TcpListenConfig, TcpMiscSockOpts, TcpSockSpeedLimitConfig,
};
use g3_types::route::HostMatch;
use g3_yaml::YamlDocPosition;

use super::{
    AnyServerConfig, ServerConfig, ServerConfigDiffAction, IDLE_CHECK_DEFAULT_DURATION,
    IDLE_CHECK_MAXIMUM_DURATION,
};

mod host;
pub(crate) use host::HttpHostConfig;

mod service;
pub(crate) use service::HttpServiceConfig;

const SERVER_CONFIG_TYPE: &str = "HttpRProxy";

/// collection of timeout config
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HttpRProxyServerTimeoutConfig {
    /// for all protocols: set the idle time to wait before recv of valid request header after all tasks done
    pub(crate) recv_req_header: Duration,
    /// for http forward only: the max time to wait after request sent before recv response header
    pub(crate) recv_rsp_header: Duration,
}

impl Default for HttpRProxyServerTimeoutConfig {
    fn default() -> Self {
        HttpRProxyServerTimeoutConfig {
            recv_req_header: Duration::from_secs(30),
            recv_rsp_header: Duration::from_secs(60),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HttpRProxyServerConfig {
    name: MetricsName,
    position: Option<YamlDocPosition>,
    pub(crate) escaper: MetricsName,
    pub(crate) user_group: MetricsName,
    pub(crate) shared_logger: Option<AsciiString>,
    pub(crate) listen: TcpListenConfig,
    pub(crate) listen_in_worker: bool,
    pub(crate) ingress_net_filter: Option<AclNetworkRuleBuilder>,
    pub(crate) server_id: Option<HttpServerId>,
    pub(crate) auth_realm: AsciiString,
    pub(crate) tcp_sock_speed_limit: TcpSockSpeedLimitConfig,
    pub(crate) timeout: HttpRProxyServerTimeoutConfig,
    pub(crate) task_idle_check_duration: Duration,
    pub(crate) task_idle_max_count: i32,
    pub(crate) tcp_copy: LimitedCopyConfig,
    pub(crate) tcp_misc_opts: TcpMiscSockOpts,
    pub(crate) req_hdr_max_size: usize,
    pub(crate) rsp_hdr_max_size: usize,
    pub(crate) log_uri_max_chars: usize,
    pub(crate) pipeline_size: usize,
    pub(crate) pipeline_read_idle_timeout: Duration,
    pub(crate) no_early_error_reply: bool,
    pub(crate) body_line_max_len: usize,
    pub(crate) http_forward_upstream_keepalive: HttpKeepAliveConfig,
    pub(crate) untrusted_read_limit: Option<TcpSockSpeedLimitConfig>,
    pub(crate) append_forwarded_for: HttpForwardedHeaderType,
    pub(crate) extra_metrics_tags: Option<Arc<StaticMetricsTags>>,
    pub(crate) hosts: HostMatch<Arc<HttpHostConfig>>,
    pub(crate) enable_tls_server: bool,
    pub(crate) global_tls_server: Option<RustlsServerConfigBuilder>,
    pub(crate) client_hello_recv_timeout: Duration,
}

impl HttpRProxyServerConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        HttpRProxyServerConfig {
            name: MetricsName::default(),
            position,
            escaper: MetricsName::default(),
            user_group: MetricsName::default(),
            shared_logger: None,
            listen: TcpListenConfig::default(),
            listen_in_worker: false,
            ingress_net_filter: None,
            server_id: None,
            auth_realm: AsciiString::from_ascii("g3proxy").unwrap(),
            tcp_sock_speed_limit: TcpSockSpeedLimitConfig::default(),
            timeout: HttpRProxyServerTimeoutConfig::default(),
            task_idle_check_duration: IDLE_CHECK_DEFAULT_DURATION,
            task_idle_max_count: 1,
            tcp_copy: Default::default(),
            tcp_misc_opts: Default::default(),
            req_hdr_max_size: 65536, // 64KiB
            rsp_hdr_max_size: 65536, // 64KiB
            log_uri_max_chars: 1024,
            pipeline_size: 10,
            pipeline_read_idle_timeout: Duration::from_secs(300),
            no_early_error_reply: false,
            body_line_max_len: 8192,
            http_forward_upstream_keepalive: Default::default(),
            untrusted_read_limit: None,
            append_forwarded_for: HttpForwardedHeaderType::default(),
            extra_metrics_tags: None,
            hosts: Default::default(),
            enable_tls_server: false,
            global_tls_server: None,
            client_hello_recv_timeout: Duration::from_secs(1),
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut server = HttpRProxyServerConfig::new(position);

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
            "user_group" => {
                self.user_group = g3_yaml::value::as_metrics_name(v)?;
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
            "server_id" => {
                let server_id = g3_yaml::value::as_http_server_id(v)
                    .context(format!("invalid http server id value for key {k}"))?;
                self.server_id = Some(server_id);
                Ok(())
            }
            "auth_realm" => {
                self.auth_realm = g3_yaml::value::as_ascii(v)
                    .context(format!("invalid ascii string value for key {k}"))?;
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
            "req_header_recv_timeout" => {
                self.timeout.recv_req_header = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "rsp_header_recv_timeout" => {
                self.timeout.recv_rsp_header = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "req_header_max_size" => {
                self.req_hdr_max_size = g3_yaml::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                Ok(())
            }
            "rsp_header_max_size" => {
                self.rsp_hdr_max_size = g3_yaml::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                Ok(())
            }
            "log_uri_max_chars" | "uri_log_max_chars" => {
                self.log_uri_max_chars = g3_yaml::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                Ok(())
            }
            "pipeline_size" => {
                self.pipeline_size = g3_yaml::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                Ok(())
            }
            "pipeline_read_idle_timeout" => {
                self.pipeline_read_idle_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "no_early_error_reply" => {
                self.no_early_error_reply = g3_yaml::value::as_bool(v)
                    .context(format!("invalid bool value for key {k}"))?;
                Ok(())
            }
            "body_line_max_length" => {
                self.body_line_max_len = g3_yaml::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                Ok(())
            }
            "http_forward_upstream_keepalive" => {
                self.http_forward_upstream_keepalive = g3_yaml::value::as_http_keepalive_config(v)
                    .context(format!("invalid http keepalive config value for key {k}"))?;
                Ok(())
            }
            "untrusted_read_speed_limit" | "untrusted_read_limit" => {
                let limit = g3_yaml::value::as_tcp_sock_speed_limit(v)
                    .context(format!("invalid tcp socket speed limit value for key {k}"))?;
                self.untrusted_read_limit = Some(limit);
                Ok(())
            }
            "append_forwarded_for" => {
                self.append_forwarded_for = g3_yaml::value::as_http_forwarded_header_type(v)
                    .context(format!(
                        "invalid http forwarded header type value for key {k}"
                    ))?;
                Ok(())
            }
            "hosts" | "sites" => {
                self.hosts = g3_yaml::value::as_host_matched_obj(v, self.position.as_ref())
                    .context(format!(
                        "invalid host matched HttpLocalSite value for key {k}"
                    ))?;
                Ok(())
            }
            "enable_tls_server" => {
                self.enable_tls_server = g3_yaml::value::as_bool(v)
                    .context(format!("invalid bool value for key {k}"))?;
                Ok(())
            }
            "global_tls_server" => {
                let lookup_dir = crate::config::get_lookup_dir(self.position.as_ref());
                let builder = g3_yaml::value::as_rustls_server_config_builder(v, Some(&lookup_dir))
                    .context(format!(
                        "invalid tls server config builder value for key {k}"
                    ))?;
                self.global_tls_server = Some(builder);
                Ok(())
            }
            "client_hello_recv_timeout" => {
                self.client_hello_recv_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
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
        if !self.user_group.is_empty() && self.auth_realm.is_empty() {
            // not really necessary as we have set default realm value
            return Err(anyhow!("auth_realm is required is auth is enabled"));
        }
        if self.task_idle_check_duration > IDLE_CHECK_MAXIMUM_DURATION {
            self.task_idle_check_duration = IDLE_CHECK_MAXIMUM_DURATION;
        }

        Ok(())
    }
}

impl ServerConfig for HttpRProxyServerConfig {
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
        &self.user_group
    }

    fn auditor(&self) -> &MetricsName {
        Default::default()
    }

    fn diff_action(&self, new: &AnyServerConfig) -> ServerConfigDiffAction {
        let new = match new {
            AnyServerConfig::HttpRProxy(config) => config,
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
