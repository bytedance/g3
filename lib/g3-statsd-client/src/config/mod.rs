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

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
use std::time::Duration;

use g3_types::metrics::MetricsName;

use crate::{StatsdClient, StatsdMetricsSink};

const UDP_DEFAULT_PORT: u16 = 8125;

#[derive(Debug, Clone)]
pub enum StatsdBackend {
    Udp(SocketAddr, Option<IpAddr>),
    Unix(PathBuf),
}

impl Default for StatsdBackend {
    fn default() -> Self {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), UDP_DEFAULT_PORT);
        StatsdBackend::Udp(addr, None)
    }
}

#[derive(Debug, Clone)]
pub struct StatsdClientConfig {
    backend: StatsdBackend,
    prefix: MetricsName,
    pub emit_duration: Duration,
}

impl Default for StatsdClientConfig {
    fn default() -> Self {
        StatsdClientConfig::with_prefix(MetricsName::default())
    }
}

impl StatsdClientConfig {
    pub fn with_prefix(prefix: MetricsName) -> Self {
        StatsdClientConfig {
            backend: StatsdBackend::default(),
            prefix,
            emit_duration: Duration::from_millis(200),
        }
    }

    pub fn set_backend(&mut self, target: StatsdBackend) {
        self.backend = target;
    }

    pub fn set_prefix(&mut self, prefix: MetricsName) {
        self.prefix = prefix;
    }

    pub fn build(&self) -> io::Result<StatsdClient> {
        let sink = match &self.backend {
            StatsdBackend::Udp(addr, bind) => {
                let bind_ip = bind.unwrap_or_else(|| match addr {
                    SocketAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                    SocketAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                });
                let socket = UdpSocket::bind(SocketAddr::new(bind_ip, 0))?;
                StatsdMetricsSink::udp_with_capacity(*addr, socket, 1024)
            }
            StatsdBackend::Unix(path) => {
                let socket = UnixDatagram::unbound()?;
                StatsdMetricsSink::unix_with_capacity(path.clone(), socket, 4096)
            }
        };

        Ok(StatsdClient::new(self.prefix.clone(), sink))
    }
}
