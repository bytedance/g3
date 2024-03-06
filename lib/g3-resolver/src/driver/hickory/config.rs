/*
 * Copyright 2024 ByteDance and/or its affiliates.
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
use std::time::Duration;

use anyhow::anyhow;

use g3_types::net::DnsEncryptionConfigBuilder;

use super::{HickoryClient, HickoryClientConfig, HickoryResolver};
use crate::driver::BoxResolverDriver;

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
    bind_ip: Option<IpAddr>,
    encryption: Option<DnsEncryptionConfigBuilder>,
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
            bind_ip: None,
            encryption: None,
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

    pub fn set_connect_timeout(&mut self, timeout: Duration) {
        self.connect_timeout = timeout;
    }

    pub fn set_request_timeout(&mut self, timeout: Duration) {
        self.request_timeout = timeout;
    }

    pub fn set_each_timeout(&mut self, timeout: Duration) {
        self.each_timeout = timeout;
    }

    pub fn set_each_tries(&mut self, attempts: i32) {
        self.each_tries = attempts;
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

    pub fn set_negative_ttl(&mut self, ttl: u32) {
        self.negative_ttl = ttl;
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
        let bind = self.bind_ip.map(|ip| SocketAddr::new(ip, 0));
        let encryption = if let Some(ec) = &self.encryption {
            Some(ec.build()?)
        } else {
            None
        };

        for ip in &self.servers {
            let client_config = HickoryClientConfig {
                target: SocketAddr::new(*ip, port),
                bind,
                encryption: encryption.clone(),
                connect_timeout: self.connect_timeout,
                request_timeout: self.request_timeout,
                each_tries: self.each_tries,
                positive_min_ttl: self.positive_min_ttl,
                positive_max_ttl: self.positive_max_ttl,
                negative_ttl: self.negative_ttl,
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
