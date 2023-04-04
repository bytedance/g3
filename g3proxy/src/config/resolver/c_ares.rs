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
use std::convert::From;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;

use anyhow::{anyhow, Context};
use yaml_rust::{yaml, Yaml};

use g3_resolver::driver::c_ares::CAresDriverConfig;
use g3_resolver::{AnyResolveDriverConfig, ResolverRuntimeConfig};
use g3_types::metrics::MetricsName;
use g3_yaml::YamlDocPosition;

use super::{AnyResolverConfig, ResolverConfigDiffAction};

const RESOLVER_CONFIG_TYPE: &str = "c-ares";

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct CAresResolverConfig {
    name: MetricsName,
    position: Option<YamlDocPosition>,
    runtime: ResolverRuntimeConfig,
    driver: CAresDriverConfig,
}

impl From<&CAresResolverConfig> for g3_resolver::ResolverConfig {
    fn from(c: &CAresResolverConfig) -> Self {
        g3_resolver::ResolverConfig {
            name: c.name.to_string(),
            runtime: c.runtime.clone(),
            driver: AnyResolveDriverConfig::CAres(c.driver.clone()),
        }
    }
}

impl CAresResolverConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        CAresResolverConfig {
            name: MetricsName::default(),
            position,
            runtime: Default::default(),
            driver: Default::default(),
        }
    }

    pub(crate) fn get_bind_ipv4(&self) -> Option<Ipv4Addr> {
        self.driver.get_bind_ipv4()
    }

    pub(crate) fn get_bind_ipv6(&self) -> Option<Ipv6Addr> {
        self.driver.get_bind_ipv6()
    }

    pub(crate) fn get_servers(&self) -> Vec<SocketAddr> {
        self.driver.get_servers()
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
            "each_timeout" => {
                self.driver.set_each_timeout(g3_yaml::value::as_u32(v)?);
                Ok(())
            }
            "each_tries" => {
                self.driver.set_each_tries(g3_yaml::value::as_u32(v)?);
                Ok(())
            }
            "round_robin" => {
                self.driver.set_round_robin(g3_yaml::value::as_bool(v)?);
                Ok(())
            }
            "socket_send_buffer_size" => {
                self.driver.set_so_send_buf_size(g3_yaml::value::as_u32(v)?);
                Ok(())
            }
            "socket_recv_buffer_size" => {
                self.driver.set_so_recv_buf_size(g3_yaml::value::as_u32(v)?);
                Ok(())
            }
            "bind_ipv4" => {
                let ip4 = g3_yaml::value::as_ipv4addr(v)?;
                self.driver.set_bind_ipv4(ip4);
                Ok(())
            }
            "bind_ipv6" => {
                let ip6 = g3_yaml::value::as_ipv6addr(v)?;
                self.driver.set_bind_ipv6(ip6);
                Ok(())
            }
            "negative_ttl" | "protective_cache_ttl" => {
                let ttl = g3_yaml::value::as_u32(v)?;
                self.driver.set_negative_ttl(ttl);
                Ok(())
            }
            "positive_ttl" | "max_cache_ttl" | "maximum_cache_ttl" => {
                let ttl = g3_yaml::value::as_u32(v)?;
                self.driver.set_positive_ttl(ttl);
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
                .context(format!("#{i} is not a valid server"))?;
        }
        Ok(())
    }

    fn parse_server_array(&mut self, addrs: &[Yaml]) -> anyhow::Result<()> {
        for (i, addr) in addrs.iter().enumerate() {
            if let Yaml::String(addr) = addr {
                self.add_server(addr)
                    .context(format!("#{i} is not a valid server"))?;
            } else {
                return Err(anyhow!("#{i} should be a string value"));
            }
        }
        Ok(())
    }

    fn parse_socket_addr(addr: &str) -> anyhow::Result<SocketAddr> {
        if let Ok(sock_addr) = SocketAddr::from_str(addr) {
            Ok(sock_addr)
        } else if let Ok(ip) = IpAddr::from_str(addr) {
            let sock_addr = SocketAddr::new(ip, 53);
            Ok(sock_addr)
        } else {
            Err(anyhow!("invalid SocketAddr / IpAddr string {addr}"))
        }
    }

    fn add_server(&mut self, addr: &str) -> anyhow::Result<()> {
        let sock_addr = CAresResolverConfig::parse_socket_addr(addr)?;
        let ip = sock_addr.ip();
        if ip.is_unspecified() {
            return Err(anyhow!("dns server address should not be unspecified"));
        }
        if ip.is_multicast() {
            return Err(anyhow!("dns server address should not be multicast"));
        }
        self.driver.add_server(sock_addr);
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

impl super::ResolverConfig for CAresResolverConfig {
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
            AnyResolverConfig::CAres(new) => new,
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
