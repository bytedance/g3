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

use anyhow::anyhow;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use c_ares_resolver::FutureResolver;
use indexmap::IndexSet;

use super::CAresResolver;
use crate::BoxResolverDriver;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CAresDriverConfig {
    flags: c_ares::Flags,
    each_timeout: u32,
    each_tries: u32,
    round_robin: bool,
    so_send_buf_size: Option<u32>,
    so_recv_buf_size: Option<u32>,
    servers: IndexSet<SocketAddr>,
    bind_v4: Option<Ipv4Addr>,
    bind_v6: Option<Ipv6Addr>,
    negative_ttl: u32,
    positive_ttl: u32,
}

impl Default for CAresDriverConfig {
    fn default() -> Self {
        CAresDriverConfig {
            flags: c_ares::Flags::empty() | c_ares::Flags::NOCHECKRESP,
            each_timeout: 5000,
            each_tries: 2,
            round_robin: false,
            so_send_buf_size: None,
            so_recv_buf_size: None,
            servers: IndexSet::new(),
            bind_v4: None,
            bind_v6: None,
            negative_ttl: crate::config::RESOLVER_MINIMUM_CACHE_TTL,
            positive_ttl: crate::config::RESOLVER_MAXIMUM_CACHE_TTL,
        }
    }
}

impl CAresDriverConfig {
    pub fn add_server(&mut self, server: SocketAddr) {
        self.servers.insert(server);
    }

    pub fn get_servers(&self) -> Vec<SocketAddr> {
        self.servers.iter().map(|addr| addr.to_owned()).collect()
    }

    pub fn get_bind_ipv4(&self) -> Option<Ipv4Addr> {
        self.bind_v4
    }

    pub fn get_bind_ipv6(&self) -> Option<Ipv6Addr> {
        self.bind_v6
    }

    pub fn set_bind_ipv4(&mut self, ip4: Ipv4Addr) {
        self.bind_v4 = Some(ip4);
    }

    pub fn set_bind_ipv6(&mut self, ip6: Ipv6Addr) {
        self.bind_v6 = Some(ip6);
    }

    pub fn set_so_send_buf_size(&mut self, size: u32) {
        self.so_send_buf_size = Some(size);
    }

    pub fn set_so_recv_buf_size(&mut self, size: u32) {
        self.so_recv_buf_size = Some(size);
    }

    pub fn set_round_robin(&mut self, enable: bool) {
        self.round_robin = enable;
    }

    pub fn set_each_timeout(&mut self, timeout_ms: u32) {
        self.each_timeout = timeout_ms;
    }

    pub fn set_each_tries(&mut self, tries: u32) {
        self.each_tries = tries;
    }

    pub fn set_negative_ttl(&mut self, ttl: u32) {
        self.negative_ttl = ttl;
    }

    pub fn set_positive_ttl(&mut self, ttl: u32) {
        self.positive_ttl = ttl;
    }

    pub fn is_unspecified(&self) -> bool {
        self.servers.is_empty()
    }

    pub(crate) fn spawn_resolver_driver(&self) -> anyhow::Result<BoxResolverDriver> {
        if self.is_unspecified() {
            return Err(anyhow!("resolver config is empty"));
        }

        let mut opts = c_ares_resolver::Options::new();
        opts.set_flags(self.flags)
            .set_timeout(self.each_timeout)
            .set_tries(self.each_tries);
        if self.round_robin {
            opts.set_rotate();
        } else {
            opts.set_no_rotate();
        }
        if let Some(size) = self.so_send_buf_size {
            opts.set_sock_send_buffer_size(size);
        }
        if let Some(size) = self.so_recv_buf_size {
            opts.set_sock_receive_buffer_size(size);
        }

        let resolver = FutureResolver::with_options(opts)
            .map_err(|e| anyhow!("failed to create resolver: {e}"))?;
        if let Some(ip4) = &self.bind_v4 {
            resolver.set_local_ipv4(*ip4);
        }
        if let Some(ip6) = &self.bind_v6 {
            resolver.set_local_ipv6(ip6);
        }
        let mut servers = Vec::<String>::new();
        for server in self.servers.iter() {
            servers.push(server.to_string());
        }
        let mut ref_servers = Vec::<&str>::new();
        for server in servers.iter() {
            ref_servers.push(server);
        }
        resolver
            .set_servers(&ref_servers)
            .map_err(|e| anyhow!("failed to set servers for resolver: {e}"))?;
        Ok(Box::new(CAresResolver {
            inner: resolver,
            negative_ttl: self.negative_ttl,
            positive_ttl: self.positive_ttl,
        }))
    }
}
