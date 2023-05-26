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

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use ascii::AsciiString;
use yaml_rust::{yaml, Yaml};

use g3_types::auth::{Password, Username};
use g3_types::collection::SelectivePickPolicy;
use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_types::net::{
    HappyEyeballsConfig, Host, HttpForwardCapability, OpensslTlsClientConfigBuilder,
    ProxyProtocolVersion, TcpKeepAliveConfig, TcpMiscSockOpts, WeightedUpstreamAddr,
};
use g3_types::resolve::{QueryStrategy, ResolveStrategy};
use g3_yaml::YamlDocPosition;

use super::{AnyEscaperConfig, EscaperConfig, EscaperConfigDiffAction, GeneralEscaperConfig};

const ESCAPER_CONFIG_TYPE: &str = "ProxyHttps";

#[derive(Clone, PartialEq)]
pub(crate) struct ProxyHttpsEscaperConfig {
    pub(crate) name: MetricsName,
    position: Option<YamlDocPosition>,
    pub(crate) shared_logger: Option<AsciiString>,
    pub(crate) proxy_nodes: Vec<WeightedUpstreamAddr>,
    pub(crate) proxy_pick_policy: SelectivePickPolicy,
    proxy_username: Username,
    proxy_password: Password,
    pub(crate) bind_v4: Option<Ipv4Addr>,
    pub(crate) bind_v6: Option<Ipv6Addr>,
    pub(crate) no_ipv4: bool,
    pub(crate) no_ipv6: bool,
    pub(crate) tls_config: OpensslTlsClientConfigBuilder,
    pub(crate) tls_name: Option<String>,
    pub(crate) resolver: MetricsName,
    pub(crate) resolve_strategy: ResolveStrategy,
    pub(crate) general: GeneralEscaperConfig,
    pub(crate) happy_eyeballs: HappyEyeballsConfig,
    pub(crate) http_forward_capability: HttpForwardCapability,
    pub(crate) tcp_keepalive: TcpKeepAliveConfig,
    pub(crate) tcp_misc_opts: TcpMiscSockOpts,
    pub(crate) http_connect_rsp_hdr_max_size: usize,
    pub(crate) append_http_headers: Vec<String>,
    pub(crate) pass_proxy_userid: bool,
    pub(crate) use_proxy_protocol: Option<ProxyProtocolVersion>,
    pub(crate) peer_negotiation_timeout: Duration,
    pub(crate) extra_metrics_tags: Option<Arc<StaticMetricsTags>>,
}

impl ProxyHttpsEscaperConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        ProxyHttpsEscaperConfig {
            name: MetricsName::default(),
            position,
            shared_logger: None,
            proxy_nodes: Vec::with_capacity(1),
            proxy_pick_policy: SelectivePickPolicy::Random,
            proxy_username: Username::empty(),
            proxy_password: Password::empty(),
            bind_v4: None,
            bind_v6: None,
            no_ipv4: false,
            no_ipv6: false,
            tls_config: OpensslTlsClientConfigBuilder::with_cache_for_many_sites(),
            tls_name: None,
            resolver: MetricsName::default(),
            resolve_strategy: Default::default(),
            general: Default::default(),
            happy_eyeballs: Default::default(),
            http_forward_capability: Default::default(),
            tcp_keepalive: Default::default(),
            tcp_misc_opts: Default::default(),
            http_connect_rsp_hdr_max_size: 4096,
            append_http_headers: Vec::new(),
            pass_proxy_userid: false,
            use_proxy_protocol: None,
            peer_negotiation_timeout: Duration::from_secs(10),
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
            "proxy_addr" => {
                match v {
                    Yaml::String(_) => {
                        let node = g3_yaml::value::as_weighted_upstream_addr(v, 3128)?;
                        self.proxy_nodes.push(node);
                    }
                    Yaml::Array(seq) => {
                        for (i, v) in seq.iter().enumerate() {
                            let node = g3_yaml::value::as_weighted_upstream_addr(v, 3128)
                                .context(format!("invalid value for {k}#{i}"))?;
                            self.proxy_nodes.push(node);
                        }
                    }
                    _ => return Err(anyhow!("invalid value type for key {k}")),
                }
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
                self.resolver = g3_yaml::value::as_metrics_name(v)?;
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
            "http_forward_capability" => {
                self.http_forward_capability = g3_yaml::value::as_http_forward_capability(v)
                    .context(format!("invalid http forward capability value for key {k}"))?;
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
                let name = g3_yaml::value::as_string(v)
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
            "http_connect_rsp_header_max_size" => {
                self.http_connect_rsp_hdr_max_size = g3_yaml::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                Ok(())
            }
            "pass_proxy_userid" => {
                self.pass_proxy_userid = g3_yaml::value::as_bool(v)
                    .context(format!("invalid bool value for key {k}"))?;
                Ok(())
            }
            "use_proxy_protocol" => {
                let version = g3_yaml::value::as_proxy_protocol_version(v)
                    .context(format!("invalid ProxyProtocolVersion value for key {k}"))?;
                self.use_proxy_protocol = Some(version);
                Ok(())
            }
            "peer_negotiation_timeout" => {
                self.peer_negotiation_timeout = g3_yaml::humanize::as_duration(v)
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
                .context("found incompatible resolver strategy".to_string())?;
            if !self.no_ipv4 && !self.no_ipv6 {
                match self.resolve_strategy.query {
                    QueryStrategy::Ipv4Only => self.no_ipv6 = true,
                    QueryStrategy::Ipv6Only => self.no_ipv4 = true,
                    _ => {}
                }
            }
        }

        if !self.proxy_username.is_empty() {
            if self.pass_proxy_userid {
                return Err(anyhow!(
                    "auth is needed for next proxy, we can not pass userid to it"
                ));
            }

            self.append_http_headers
                .push(g3_http::header::proxy_authorization_basic(
                    &self.proxy_username,
                    &self.proxy_password,
                ));
        }

        Ok(())
    }
}

impl EscaperConfig for ProxyHttpsEscaperConfig {
    fn name(&self) -> &MetricsName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn escaper_type(&self) -> &str {
        ESCAPER_CONFIG_TYPE
    }

    fn resolver(&self) -> &MetricsName {
        &self.resolver
    }

    fn diff_action(&self, new: &AnyEscaperConfig) -> EscaperConfigDiffAction {
        let new = match new {
            AnyEscaperConfig::ProxyHttps(config) => config,
            _ => return EscaperConfigDiffAction::SpawnNew,
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
