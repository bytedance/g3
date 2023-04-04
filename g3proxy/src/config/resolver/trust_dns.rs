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

use std::collections::BTreeSet;
use std::net::IpAddr;
use std::str::FromStr;

use anyhow::{anyhow, Context};
use yaml_rust::{yaml, Yaml};

use g3_resolver::driver::trust_dns::TrustDnsDriverConfig;
use g3_resolver::{AnyResolveDriverConfig, ResolverRuntimeConfig};
use g3_types::metrics::MetricsName;
use g3_yaml::YamlDocPosition;

use super::{AnyResolverConfig, ResolverConfigDiffAction};

const RESOLVER_CONFIG_TYPE: &str = "trust-dns";

#[derive(Clone, PartialEq)]
pub(crate) struct TrustDnsResolverConfig {
    name: MetricsName,
    position: Option<YamlDocPosition>,
    runtime: ResolverRuntimeConfig,
    driver: TrustDnsDriverConfig,
}

impl From<&TrustDnsResolverConfig> for g3_resolver::ResolverConfig {
    fn from(c: &TrustDnsResolverConfig) -> Self {
        g3_resolver::ResolverConfig {
            name: c.name.to_string(),
            runtime: c.runtime.clone(),
            driver: AnyResolveDriverConfig::TrustDns(c.driver.clone()),
        }
    }
}

impl TrustDnsResolverConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        TrustDnsResolverConfig {
            name: MetricsName::default(),
            position,
            runtime: Default::default(),
            driver: Default::default(),
        }
    }

    #[inline]
    pub(crate) fn get_bind_ip(&self) -> Option<IpAddr> {
        self.driver.get_bind_ip()
    }

    #[inline]
    pub(crate) fn get_servers(&self) -> Vec<IpAddr> {
        self.driver.get_servers()
    }

    #[inline]
    pub(crate) fn get_server_port(&self) -> Option<u16> {
        self.driver.get_server_port()
    }

    pub(crate) fn get_encryption_summary(&self) -> Option<String> {
        self.driver.get_encryption().map(|c| c.summary())
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut resolver = Self::new(position);

        g3_yaml::foreach_kv(map, |k, v| resolver.set(k, v))?;

        resolver.check()?;
        Ok(resolver)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_RESOLVER_TYPE => Ok(()),
            super::CONFIG_KEY_RESOLVER_NAME => {
                self.name = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "server" => match v {
                Yaml::String(addrs) => self.parse_server_str(addrs),
                Yaml::Array(seq) => self.parse_server_array(seq),
                _ => Err(anyhow!("invalid yaml value type, expect string / array")),
            },
            "server_port" => {
                let port = g3_yaml::value::as_u16(v)?;
                self.driver.set_server_port(port);
                Ok(())
            }
            "encryption" | "encrypt" => {
                let lookup_dir = crate::config::get_lookup_dir(self.position.as_ref());
                let config =
                    g3_yaml::value::as_dns_encryption_protocol_builder(v, Some(&lookup_dir))
                        .context(format!("invalid dns encryption config value for key {k}"))?;
                self.driver.set_encryption(config);
                Ok(())
            }
            "each_timeout" => {
                let timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                self.driver.set_each_timeout(timeout);
                Ok(())
            }
            "retry_attempts" => {
                let attempts = g3_yaml::value::as_usize(v)?;
                self.driver.set_retry_attempts(attempts);
                Ok(())
            }
            "bind_ip" => {
                let ip = g3_yaml::value::as_ipaddr(v)?;
                self.driver.set_bind_ip(ip);
                Ok(())
            }
            "positive_min_ttl" => {
                let ttl = g3_yaml::value::as_u32(v)?;
                self.driver.set_positive_min_ttl(ttl);
                Ok(())
            }
            "positive_max_ttl" => {
                let ttl = g3_yaml::value::as_u32(v)?;
                self.driver.set_positive_max_ttl(ttl);
                Ok(())
            }
            "negative_min_ttl" => {
                let ttl = g3_yaml::value::as_u32(v)?;
                self.driver.set_negative_min_ttl(ttl);
                Ok(())
            }
            "negative_max_ttl" => {
                let ttl = g3_yaml::value::as_u32(v)?;
                self.driver.set_negative_max_ttl(ttl);
                Ok(())
            }
            "graceful_stop_wait" => {
                self.runtime.graceful_stop_wait = g3_yaml::humanize::as_duration(v)?;
                Ok(())
            }
            "protective_query_timeout" => {
                self.runtime.protective_query_timeout = g3_yaml::humanize::as_duration(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn parse_server_str(&mut self, addrs: &str) -> anyhow::Result<()> {
        let addrs = addrs.split_whitespace();
        for (i, addr) in addrs.enumerate() {
            self.add_server(addr)
                .context(format!("#{i} is not a valid ip address"))?;
        }
        Ok(())
    }

    fn parse_server_array(&mut self, addrs: &[Yaml]) -> anyhow::Result<()> {
        for (i, addr) in addrs.iter().enumerate() {
            if let Yaml::String(addr) = addr {
                self.add_server(addr)
                    .context(format!("#{i} is not a valid ip address"))?;
            } else {
                return Err(anyhow!("#{i} should be a string value"));
            }
        }
        Ok(())
    }

    fn add_server(&mut self, addr: &str) -> anyhow::Result<()> {
        let ip = IpAddr::from_str(addr)?;
        self.driver.add_server(ip);
        Ok(())
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.driver.is_unspecified() {
            return Err(anyhow!("no dns server has been set"));
        }

        Ok(())
    }
}

impl super::ResolverConfig for TrustDnsResolverConfig {
    fn name(&self) -> &MetricsName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn resolver_type(&self) -> &'static str {
        RESOLVER_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyResolverConfig) -> ResolverConfigDiffAction {
        let new = match new {
            AnyResolverConfig::TrustDns(new) => new,
            _ => return ResolverConfigDiffAction::SpawnNew,
        };

        if self.eq(new) {
            return ResolverConfigDiffAction::NoAction;
        }

        ResolverConfigDiffAction::Update
    }

    fn dependent_resolver(&self) -> Option<BTreeSet<MetricsName>> {
        None
    }
}
