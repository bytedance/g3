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

use std::io::{self, Write};
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Context, anyhow};
use chrono::{DateTime, Utc};
use http::uri::PathAndQuery;
use log::warn;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use yaml_rust::Yaml;

use g3_socket::BindAddr;
use g3_types::metrics::NodeName;
use g3_types::net::{Host, UpstreamAddr};

use super::{HttpExport, HttpExportRuntime};
use crate::types::MetricRecord;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HttpExportConfig {
    pub(super) exporter: NodeName,
    pub(super) server: Host,
    port: u16,
    pub(super) api_path: PathAndQuery,
    resolve_retry_wait: Duration,
    connect_retry_wait: Duration,

    peer_s: String,
    peer_addrs: Vec<SocketAddr>,
}

impl HttpExportConfig {
    pub(crate) fn new(default_port: u16, default_path: &'static str) -> Self {
        HttpExportConfig {
            exporter: NodeName::default(),
            server: Host::empty(),
            port: default_port,
            api_path: PathAndQuery::from_static(default_path),
            resolve_retry_wait: Duration::from_secs(30),
            connect_retry_wait: Duration::from_secs(10),
            peer_s: String::new(),
            peer_addrs: Vec::new(),
        }
    }

    pub(crate) fn check(&mut self, exporter: NodeName) -> anyhow::Result<()> {
        if self.server.is_empty() {
            return Err(anyhow!("peer address is not set"));
        }

        self.exporter = exporter;
        let peer = UpstreamAddr::new(self.server.clone(), self.port);
        self.peer_s = peer.to_string();
        Ok(())
    }

    pub(crate) fn set_by_yaml_kv(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "server" => {
                self.server = g3_yaml::value::as_host(v)?;
                Ok(())
            }
            "port" => {
                self.port = g3_yaml::value::as_u16(v)?;
                Ok(())
            }
            "api_path" => {
                self.api_path = g3_yaml::value::as_http_path_and_query(v)
                    .context(format!("invalid http path_query value for key {k}"))?;
                Ok(())
            }
            "resolve_retry_wait" => {
                self.resolve_retry_wait = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "connect_retry_wait" => {
                self.connect_retry_wait = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    async fn select_peer(&mut self) -> Option<SocketAddr> {
        match tokio::net::lookup_host(&self.peer_s).await {
            Ok(peers) => {
                self.peer_addrs.clear();
                self.peer_addrs.extend(peers);
            }
            Err(e) => {
                warn!(
                    "exporter {}: failed to resolve {}: {e}",
                    self.exporter, self.peer_s
                );
            }
        }

        fastrand::choice(&self.peer_addrs).cloned()
    }

    async fn connect_peer(&self, peer: SocketAddr) -> io::Result<TcpStream> {
        let socket = g3_socket::tcp::new_socket_to(
            peer.ip(),
            &BindAddr::None,
            &Default::default(),
            &Default::default(),
            false,
        )?;
        socket.connect(peer).await
    }

    pub(super) async fn connect(&mut self) -> Result<TcpStream, Duration> {
        let Some(peer) = self.select_peer().await else {
            return Err(self.resolve_retry_wait);
        };

        match self.connect_peer(peer).await {
            Ok(stream) => Ok(stream),
            Err(e) => {
                warn!(
                    "exporter {}: failed to connect to {peer}: {e}",
                    self.exporter
                );
                Err(self.connect_retry_wait)
            }
        }
    }

    pub(super) fn write_fixed_header(&self, header_buf: &mut Vec<u8>) {
        header_buf.extend_from_slice(b"POST ");
        header_buf.extend_from_slice(self.api_path.as_str().as_bytes());
        header_buf.extend_from_slice(b" HTTP/1.1\r\n");
        header_buf.extend_from_slice(b"Host: ");
        let _ = write!(header_buf, "{}", self.server);
        header_buf.extend_from_slice(b"\r\n");
        header_buf.extend_from_slice(b"Connection: keep-alive\r\n");
    }

    pub(crate) fn spawn<T>(&self, formatter: T) -> mpsc::Sender<(DateTime<Utc>, MetricRecord)>
    where
        T: HttpExport + Send + Sync + 'static,
    {
        let (sender, receiver) = mpsc::channel(1024);

        let runtime = HttpExportRuntime::new(self.clone(), formatter, receiver);
        tokio::spawn(runtime.into_running());

        sender
    }
}
