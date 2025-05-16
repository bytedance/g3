/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::io::{self, Write};
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Context, anyhow};
use http::HeaderMap;
use http::uri::PathAndQuery;
use log::warn;
use tokio::net::TcpStream;
use yaml_rust::Yaml;

use g3_socket::BindAddr;
use g3_types::metrics::NodeName;
use g3_types::net::{Host, UpstreamAddr};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HttpExportConfig {
    pub(super) exporter: NodeName,
    pub(super) host: Host,
    port: u16,
    resolve_retry_wait: Duration,
    connect_retry_wait: Duration,
    pub(super) rsp_head_max_size: usize,
    pub(super) body_line_max_len: usize,

    peer_s: String,
    peer_addrs: Vec<SocketAddr>,
}

impl HttpExportConfig {
    pub(crate) fn new(default_port: u16) -> Self {
        HttpExportConfig::with_server(Host::empty(), default_port)
    }

    pub(crate) fn with_server(host: Host, port: u16) -> Self {
        HttpExportConfig {
            exporter: NodeName::default(),
            host,
            port,
            resolve_retry_wait: Duration::from_secs(30),
            connect_retry_wait: Duration::from_secs(10),
            rsp_head_max_size: 8192,
            body_line_max_len: 512,
            peer_s: String::new(),
            peer_addrs: Vec::new(),
        }
    }

    pub(crate) fn check(&mut self, exporter: NodeName) -> anyhow::Result<()> {
        if self.host.is_empty() {
            return Err(anyhow!("peer address is not set"));
        }

        self.exporter = exporter;
        let peer = UpstreamAddr::new(self.host.clone(), self.port);
        self.peer_s = peer.to_string();
        Ok(())
    }

    pub(crate) fn set_by_yaml_kv(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "host" | "server" => {
                self.host = g3_yaml::value::as_host(v)?;
                Ok(())
            }
            "port" => {
                self.port = g3_yaml::value::as_u16(v)?;
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
            "rsp_header_max_size" => {
                self.rsp_head_max_size = g3_yaml::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                Ok(())
            }
            "body_line_max_length" => {
                self.body_line_max_len = g3_yaml::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
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

    pub(super) fn write_fixed_header(
        &self,
        api_path: &PathAndQuery,
        header_buf: &mut Vec<u8>,
        static_headers: &HeaderMap,
    ) {
        header_buf.extend_from_slice(b"POST ");
        header_buf.extend_from_slice(api_path.as_str().as_bytes());
        header_buf.extend_from_slice(b" HTTP/1.1\r\n");
        header_buf.extend_from_slice(b"Host: ");
        let _ = write!(header_buf, "{}", self.host);
        header_buf.extend_from_slice(b"\r\n");
        header_buf.extend_from_slice(b"Connection: keep-alive\r\n");
        for (header, value) in static_headers {
            header_buf.extend_from_slice(header.as_str().as_bytes());
            header_buf.extend_from_slice(b": ");
            header_buf.extend_from_slice(value.as_bytes());
            header_buf.extend_from_slice(b"\r\n");
        }
    }
}
