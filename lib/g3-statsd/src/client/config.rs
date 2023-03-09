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

use std::net::{IpAddr, Ipv6Addr, SocketAddr, UdpSocket};
use std::os::unix::net::UnixDatagram;
use std::time::Duration;

use cadence::{BufferedUdpMetricSink, BufferedUnixMetricSink, StatsdClient, StatsdClientBuilder};

use g3_types::metrics::MetricsName;

use super::{StatsdBackend, StatsdClientBuildError};

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

    pub fn build(&self) -> Result<StatsdClientBuilder, StatsdClientBuildError> {
        let builder = match &self.backend {
            StatsdBackend::Udp(addr, bind) => {
                let bind_ip = bind.unwrap_or(IpAddr::V6(Ipv6Addr::UNSPECIFIED));
                let socket = UdpSocket::bind(SocketAddr::new(bind_ip, 0))
                    .map_err(StatsdClientBuildError::SocketError)?;
                let sink = BufferedUdpMetricSink::with_capacity(addr, socket, 1024)
                    .map_err(StatsdClientBuildError::SinkError)?;
                StatsdClient::builder(self.prefix.as_str(), sink)
            }
            StatsdBackend::Unix(path) => {
                let socket =
                    UnixDatagram::unbound().map_err(StatsdClientBuildError::SocketError)?;
                let sink = BufferedUnixMetricSink::with_capacity(path, socket, 4096);
                StatsdClient::builder(self.prefix.as_str(), sink)
            }
        };

        Ok(builder)
    }
}
