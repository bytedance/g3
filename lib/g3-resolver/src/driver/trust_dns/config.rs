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

use anyhow::{anyhow, Context};
use std::convert::TryFrom;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use rustls::ServerName;
use trust_dns_resolver::config::{NameServerConfigGroup, ResolverConfig, ResolverOpts};
use trust_dns_resolver::TokioAsyncResolver;

use g3_types::net::{DnsEncryptionConfigBuilder, DnsEncryptionProtocol};

use super::TrustDnsResolver;
use crate::BoxResolverDriver;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustDnsDriverConfig {
    each_timeout: Duration,
    retry_attempts: usize,
    positive_min_ttl: u32,
    positive_max_ttl: u32,
    negative_min_ttl: u32,
    negative_max_ttl: u32,
    servers: Vec<IpAddr>,
    server_port: Option<u16>,
    bind_ip: Option<IpAddr>,
    encryption: Option<DnsEncryptionConfigBuilder>,
}

impl Default for TrustDnsDriverConfig {
    fn default() -> Self {
        TrustDnsDriverConfig {
            each_timeout: Duration::from_secs(5),
            retry_attempts: 2,
            positive_min_ttl: crate::config::RESOLVER_MINIMUM_CACHE_TTL,
            positive_max_ttl: crate::config::RESOLVER_MAXIMUM_CACHE_TTL,
            negative_min_ttl: crate::config::RESOLVER_MINIMUM_CACHE_TTL,
            negative_max_ttl: crate::config::RESOLVER_MAXIMUM_CACHE_TTL,
            servers: vec![],
            server_port: None,
            bind_ip: None,
            encryption: None,
        }
    }
}

impl From<&TrustDnsDriverConfig> for ResolverOpts {
    fn from(c: &TrustDnsDriverConfig) -> Self {
        let mut opts = ResolverOpts::default();
        opts.timeout = c.each_timeout;
        opts.attempts = c.retry_attempts;
        opts.cache_size = 0;
        opts.use_hosts_file = false;
        opts.positive_min_ttl = Some(Duration::from_secs(c.positive_min_ttl as u64));
        opts.negative_min_ttl = Some(Duration::from_secs(c.negative_min_ttl as u64));
        opts.positive_max_ttl = Some(Duration::from_secs(c.positive_max_ttl as u64));
        opts.negative_max_ttl = Some(Duration::from_secs(c.negative_max_ttl as u64));
        opts.preserve_intermediates = false;
        opts
    }
}

impl TryFrom<&TrustDnsDriverConfig> for NameServerConfigGroup {
    type Error = anyhow::Error;

    fn try_from(c: &TrustDnsDriverConfig) -> anyhow::Result<Self> {
        let g = if let Some(ec) = &c.encryption {
            let tls_name = match ec.tls_name() {
                ServerName::DnsName(n) => n.as_ref().to_string(),
                ServerName::IpAddress(ip) => ip.to_string(),
                v => return Err(anyhow!("unsupported tls server name: {v:?}")), // FIXME add after trust-dns support it
            };

            let mut g = match ec.protocol() {
                DnsEncryptionProtocol::Tls => NameServerConfigGroup::from_ips_tls(
                    &c.servers,
                    c.server_port.unwrap_or(853),
                    tls_name,
                    false,
                ),
                DnsEncryptionProtocol::Https => NameServerConfigGroup::from_ips_https(
                    &c.servers,
                    c.server_port.unwrap_or(443),
                    tls_name,
                    false,
                ),
            };

            if let Some(config) = ec
                .build_tls_client_config()
                .context("unable to build tls client config")?
            {
                g = g.with_client_config(config.driver);
            }

            g
        } else {
            NameServerConfigGroup::from_ips_clear(&c.servers, c.server_port.unwrap_or(53), false)
        };

        if let Some(ip) = &c.bind_ip {
            Ok(g.with_bind_addr(Some(SocketAddr::new(*ip, 0))))
        } else {
            Ok(g)
        }
    }
}

impl TrustDnsDriverConfig {
    pub fn add_server(&mut self, ip: IpAddr) {
        self.servers.push(ip);
    }

    #[inline]
    pub fn get_servers(&self) -> Vec<IpAddr> {
        self.servers.clone()
    }

    pub fn set_server_port(&mut self, port: u16) {
        self.server_port = Some(port);
    }

    #[inline]
    pub fn get_server_port(&self) -> Option<u16> {
        self.server_port
    }

    pub fn set_encryption(&mut self, config: DnsEncryptionConfigBuilder) {
        self.encryption = Some(config);
    }

    #[inline]
    pub fn get_encryption(&self) -> Option<&DnsEncryptionConfigBuilder> {
        self.encryption.as_ref()
    }

    pub fn set_each_timeout(&mut self, timeout: Duration) {
        self.each_timeout = timeout;
    }

    pub fn set_retry_attempts(&mut self, attempts: usize) {
        self.retry_attempts = attempts;
    }

    pub fn set_bind_ip(&mut self, ip: IpAddr) {
        self.bind_ip = Some(ip);
    }

    #[inline]
    pub fn get_bind_ip(&self) -> Option<IpAddr> {
        self.bind_ip
    }

    pub fn set_positive_min_ttl(&mut self, ttl: u32) {
        self.positive_min_ttl = ttl;
    }

    pub fn set_positive_max_ttl(&mut self, ttl: u32) {
        self.positive_max_ttl = ttl;
    }

    pub fn set_negative_min_ttl(&mut self, ttl: u32) {
        self.negative_min_ttl = ttl;
    }

    pub fn set_negative_max_ttl(&mut self, ttl: u32) {
        self.negative_max_ttl = ttl;
    }

    pub fn is_unspecified(&self) -> bool {
        self.servers.is_empty()
    }

    pub(crate) fn spawn_resolver_driver(&self) -> anyhow::Result<BoxResolverDriver> {
        let name_servers = NameServerConfigGroup::try_from(self)?;
        let d_config = ResolverConfig::from_parts(None, vec![], name_servers);
        let d_opts = ResolverOpts::from(self);

        let d_resolver = TokioAsyncResolver::tokio(d_config, d_opts)
            .map_err(|e| anyhow!("failed to create resolver: {e}"))?;

        let resolver = TrustDnsResolver {
            inner: Arc::new(d_resolver),
            protective_cache_ttl: self.negative_min_ttl,
        };
        Ok(Box::new(resolver))
    }
}
