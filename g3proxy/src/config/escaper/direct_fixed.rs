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

use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use ascii::AsciiString;
use yaml_rust::{yaml, Yaml};

use g3_types::acl::{AclAction, AclNetworkRuleBuilder};
use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_types::net::{HappyEyeballsConfig, TcpKeepAliveConfig, TcpMiscSockOpts, UdpMiscSockOpts};
use g3_types::resolve::{QueryStrategy, ResolveRedirectionBuilder, ResolveStrategy};
use g3_yaml::YamlDocPosition;

use super::{AnyEscaperConfig, EscaperConfig, EscaperConfigDiffAction, GeneralEscaperConfig};

const ESCAPER_CONFIG_TYPE: &str = "DirectFixed";

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct DirectFixedEscaperConfig {
    pub(crate) name: String,
    position: Option<YamlDocPosition>,
    pub(crate) shared_logger: Option<AsciiString>,
    pub(crate) bind4: Vec<IpAddr>,
    pub(crate) bind6: Vec<IpAddr>,
    pub(crate) no_ipv4: bool,
    pub(crate) no_ipv6: bool,
    pub(crate) resolver: MetricsName,
    pub(crate) resolve_strategy: ResolveStrategy,
    pub(crate) resolve_redirection: Option<ResolveRedirectionBuilder>,
    pub(crate) egress_net_filter: AclNetworkRuleBuilder,
    pub(crate) general: GeneralEscaperConfig,
    pub(crate) happy_eyeballs: HappyEyeballsConfig,
    pub(crate) tcp_keepalive: TcpKeepAliveConfig,
    pub(crate) tcp_misc_opts: TcpMiscSockOpts,
    pub(crate) udp_misc_opts: UdpMiscSockOpts,
    pub(crate) enable_path_selection: bool,
    pub(crate) extra_metrics_tags: Option<Arc<StaticMetricsTags>>,
}

impl DirectFixedEscaperConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        DirectFixedEscaperConfig {
            name: String::new(),
            position,
            shared_logger: None,
            bind4: Vec::new(),
            bind6: Vec::new(),
            no_ipv4: false,
            no_ipv6: false,
            resolver: MetricsName::default(),
            resolve_strategy: Default::default(),
            resolve_redirection: None,
            egress_net_filter: AclNetworkRuleBuilder::new_egress(AclAction::Permit),
            general: Default::default(),
            happy_eyeballs: Default::default(),
            tcp_keepalive: Default::default(),
            tcp_misc_opts: Default::default(),
            udp_misc_opts: Default::default(),
            enable_path_selection: false,
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
                if let Yaml::String(name) = v {
                    self.name.clone_from(name);
                    Ok(())
                } else {
                    Err(anyhow!("invalid string value for key {k}"))
                }
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
            "bind_ip" => match v {
                Yaml::String(ip) => self.add_bind_address(ip),
                Yaml::Array(seq) => {
                    for (i, ip) in seq.iter().enumerate() {
                        if let Yaml::String(ip) = ip {
                            self.add_bind_address(ip)?;
                        } else {
                            return Err(anyhow!("invalid string value for {k}#{i}"));
                        }
                    }
                    Ok(())
                }
                _ => Err(anyhow!("invalid value type for key {k}")),
            },
            "resolver" => {
                self.resolver = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "resolve_strategy" => {
                self.resolve_strategy = g3_yaml::value::as_resolve_strategy(v)
                    .context(format!("invalid resolve strategy value for key {k}"))?;
                Ok(())
            }
            "resolve_redirection" => {
                let redirect = g3_yaml::value::as_resolve_redirection_builder(v)
                    .context(format!("invalid resolve redirection value for key {k}"))?;
                self.resolve_redirection = Some(redirect);
                Ok(())
            }
            "enable_path_selection" => {
                self.enable_path_selection = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "egress_network_filter" | "egress_net_filter" => {
                self.egress_net_filter = g3_yaml::value::acl::as_egress_network_rule_builder(v)
                    .context(format!("invalid network acl rule value for key {k}"))?;
                Ok(())
            }
            "tcp_sock_speed_limit" | "tcp_conn_speed_limit" | "tcp_conn_limit" => {
                self.general.tcp_sock_speed_limit = g3_yaml::value::as_tcp_sock_speed_limit(v)
                    .context(format!("invalid tcp conn socket limit value for key {k}"))?;
                Ok(())
            }
            "udp_sock_speed_limit" | "udp_relay_speed_limit" | "udp_relay_limit" => {
                self.general.udp_sock_speed_limit = g3_yaml::value::as_udp_sock_speed_limit(v)
                    .context(format!("invalid udp socket speed limit value for key {k}"))?;
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
        if self.resolver.is_empty() {
            return Err(anyhow!("resolver is not set"));
        }
        if self.no_ipv4 && self.no_ipv6 {
            return Err(anyhow!("both ipv4 and ipv6 are disabled"));
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

        Ok(())
    }

    fn add_bind_address(&mut self, ip: &str) -> anyhow::Result<()> {
        let ip = IpAddr::from_str(ip)?;
        match ip {
            IpAddr::V4(_) => self.bind4.push(ip),
            IpAddr::V6(_) => self.bind6.push(ip),
        }
        Ok(())
    }
}

impl EscaperConfig for DirectFixedEscaperConfig {
    fn name(&self) -> &str {
        self.name.as_str()
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
            AnyEscaperConfig::DirectFixed(config) => config,
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
