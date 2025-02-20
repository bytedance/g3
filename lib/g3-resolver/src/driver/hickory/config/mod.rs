/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_socket::BindAddr;
use g3_types::net::{DnsEncryptionConfigBuilder, TcpMiscSockOpts, UdpMiscSockOpts};

use super::{HickoryClient, HickoryClientConfig, HickoryResolver};
use crate::driver::BoxResolverDriver;

#[cfg(feature = "yaml")]
mod yaml;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HickoryDriverConfig {
    connect_timeout: Duration,
    request_timeout: Duration,
    each_timeout: Duration,
    each_tries: i32,
    retry_interval: Duration,
    positive_min_ttl: u32,
    positive_max_ttl: u32,
    negative_ttl: u32,
    servers: Vec<IpAddr>,
    server_port: Option<u16>,
    bind_addr: BindAddr,
    encryption: Option<DnsEncryptionConfigBuilder>,
    tcp_misc_opts: TcpMiscSockOpts,
    udp_misc_opts: UdpMiscSockOpts,
}

impl Default for HickoryDriverConfig {
    fn default() -> Self {
        HickoryDriverConfig {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(5),
            each_timeout: Duration::from_secs(5),
            each_tries: 2,
            retry_interval: Duration::from_secs(1),
            positive_min_ttl: crate::config::RESOLVER_MINIMUM_CACHE_TTL,
            positive_max_ttl: crate::config::RESOLVER_MAXIMUM_CACHE_TTL,
            negative_ttl: crate::config::RESOLVER_MINIMUM_CACHE_TTL,
            servers: vec![],
            server_port: None,
            bind_addr: BindAddr::None,
            encryption: None,
            tcp_misc_opts: Default::default(),
            udp_misc_opts: Default::default(),
        }
    }
}

impl HickoryDriverConfig {
    pub fn check(&mut self) -> anyhow::Result<()> {
        if self.servers.is_empty() {
            return Err(anyhow!("no dns server set"));
        }
        if self.positive_max_ttl < self.positive_min_ttl {
            self.positive_max_ttl = self.positive_min_ttl;
        }

        Ok(())
    }

    fn parse_server_str(&mut self, addrs: &str) -> anyhow::Result<()> {
        let addrs = addrs.split_whitespace();
        for (i, addr) in addrs.enumerate() {
            self.add_server_str(addr)
                .context(format!("#{i} is not a valid ip address"))?;
        }
        Ok(())
    }

    fn parse_server_array(&mut self, addrs: &[Yaml]) -> anyhow::Result<()> {
        for (i, addr) in addrs.iter().enumerate() {
            if let Yaml::String(addr) = addr {
                self.add_server_str(addr)
                    .context(format!("#{i} is not a valid ip address"))?;
            } else {
                return Err(anyhow!("#{i} should be a string value"));
            }
        }
        Ok(())
    }

    fn add_server_str(&mut self, addr: &str) -> anyhow::Result<()> {
        let ip = IpAddr::from_str(addr)?;
        self.servers.push(ip);
        Ok(())
    }

    #[inline]
    pub fn get_servers(&self) -> Vec<IpAddr> {
        self.servers.clone()
    }

    #[inline]
    pub fn get_server_port(&self) -> Option<u16> {
        self.server_port
    }

    #[inline]
    pub fn get_encryption(&self) -> Option<&DnsEncryptionConfigBuilder> {
        self.encryption.as_ref()
    }

    #[inline]
    pub fn get_bind_addr(&self) -> BindAddr {
        self.bind_addr
    }

    pub(crate) fn spawn_resolver_driver(&self) -> anyhow::Result<BoxResolverDriver> {
        let mut driver =
            HickoryResolver::new(self.each_timeout, self.retry_interval, self.negative_ttl);
        let port = self.server_port.unwrap_or_else(|| {
            self.encryption
                .as_ref()
                .map(|v| v.protocol().default_port())
                .unwrap_or(53)
        });
        let encryption = if let Some(ec) = &self.encryption {
            Some(ec.build()?)
        } else {
            None
        };

        for ip in &self.servers {
            let client_config = HickoryClientConfig {
                target: SocketAddr::new(*ip, port),
                bind: self.bind_addr,
                encryption: encryption.clone(),
                connect_timeout: self.connect_timeout,
                request_timeout: self.request_timeout,
                each_tries: self.each_tries,
                positive_min_ttl: self.positive_min_ttl,
                positive_max_ttl: self.positive_max_ttl,
                negative_ttl: self.negative_ttl,
                tcp_misc_opts: self.tcp_misc_opts,
                udp_misc_opts: self.udp_misc_opts,
            };
            let (req_sender, req_receiver) = flume::unbounded();
            driver.push_client(req_sender);
            tokio::spawn(async move {
                let client = HickoryClient::new(client_config).await.unwrap(); // TODO
                client.run(req_receiver).await;
            });
        }

        Ok(Box::new(driver))
    }
}
