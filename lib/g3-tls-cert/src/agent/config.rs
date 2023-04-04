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
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use tokio::net::UdpSocket;

use g3_types::net::SocketBufferConfig;

use super::{CertAgentHandle, QueryRuntime};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertAgentConfig {
    pub(crate) cache_request_batch_count: usize,
    pub(crate) cache_request_timeout: Duration,
    pub(crate) cache_vanish_wait: Duration,
    pub(crate) query_peer_addr: SocketAddr,
    pub(crate) query_socket_buffer: SocketBufferConfig,
    pub(crate) query_wait_timeout: Duration,
    pub(crate) protective_cache_ttl: u32,
    pub(crate) maximum_cache_ttl: u32,
}

impl Default for CertAgentConfig {
    fn default() -> Self {
        CertAgentConfig {
            cache_request_batch_count: 10,
            cache_request_timeout: Duration::from_millis(800),
            cache_vanish_wait: Duration::from_secs(300),
            query_peer_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 2999),
            query_socket_buffer: SocketBufferConfig::default(),
            query_wait_timeout: Duration::from_millis(400),
            protective_cache_ttl: 10,
            maximum_cache_ttl: 300,
        }
    }
}

impl CertAgentConfig {
    pub fn set_cache_request_batch_count(&mut self, count: usize) {
        self.cache_request_batch_count = count;
    }

    pub fn set_cache_request_timeout(&mut self, time: Duration) {
        self.cache_request_timeout = time;
    }

    pub fn set_cache_vanish_wait(&mut self, time: Duration) {
        self.cache_vanish_wait = time;
    }

    pub fn set_query_peer_addr(&mut self, addr: SocketAddr) {
        self.query_peer_addr = addr;
    }

    pub fn set_query_socket_buffer(&mut self, config: SocketBufferConfig) {
        self.query_socket_buffer = config;
    }

    pub fn set_query_wait_timeout(&mut self, time: Duration) {
        self.query_wait_timeout = time;
    }

    pub fn set_protective_cache_ttl(&mut self, ttl: u32) {
        self.protective_cache_ttl = ttl;
    }

    pub fn set_maximum_cache_ttl(&mut self, ttl: u32) {
        self.maximum_cache_ttl = ttl;
    }

    pub fn spawn_cert_agent(&self) -> anyhow::Result<CertAgentHandle> {
        use anyhow::Context;

        let (socket, _addr) = g3_socket::udp::new_std_bind_connect(
            None,
            self.query_socket_buffer,
            &Default::default(),
        )
        .context("failed to setup udp socket")?;
        socket.connect(self.query_peer_addr).map_err(|e| {
            anyhow!(
                "failed to connect to peer address {}: {e:?}",
                self.query_peer_addr
            )
        })?;
        let socket = UdpSocket::from_std(socket).context("failed to setup udp socket")?;

        let (cache_runtime, cache_handle, query_handle) =
            g3_io_ext::spawn_effective_cache(self.cache_request_batch_count);
        let query_runtime = QueryRuntime::new(self, socket, query_handle);

        tokio::spawn(query_runtime);
        tokio::spawn(cache_runtime);

        Ok(CertAgentHandle::new(
            cache_handle,
            self.cache_request_timeout,
        ))
    }
}
