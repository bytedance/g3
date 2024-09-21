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

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::anyhow;
use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use rustls_pki_types::ServerName;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::{tcp, TcpStream};
use tokio::sync::broadcast;
use tokio::time::Instant;
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;

use g3_io_ext::AsyncStream;
use g3_types::collection::{SelectiveVec, WeightedValue};
use g3_types::ext::DurationExt;
use g3_types::net::RustlsClientConfig;

use crate::config::backend::keyless_tcp::KeylessTcpBackendConfig;
use crate::module::keyless::{
    KeylessBackendStats, KeylessForwardRequest, KeylessUpstreamConnect,
    KeylessUpstreamDurationRecorder, MultiplexedUpstreamConnection,
};

pub(super) struct KeylessTcpUpstreamConnector {
    config: Arc<KeylessTcpBackendConfig>,
    stats: Arc<KeylessBackendStats>,
    duration_recorder: Arc<KeylessUpstreamDurationRecorder>,
    peer_addrs: Arc<ArcSwapOption<SelectiveVec<WeightedValue<SocketAddr>>>>,
}

impl KeylessTcpUpstreamConnector {
    pub(super) fn new(
        config: Arc<KeylessTcpBackendConfig>,
        site_stats: Arc<KeylessBackendStats>,
        duration_recorder: Arc<KeylessUpstreamDurationRecorder>,
        peer_addrs_container: Arc<ArcSwapOption<SelectiveVec<WeightedValue<SocketAddr>>>>,
    ) -> Self {
        KeylessTcpUpstreamConnector {
            config,
            stats: site_stats,
            duration_recorder,
            peer_addrs: peer_addrs_container,
        }
    }

    async fn connect(&self) -> anyhow::Result<(TcpStream, SocketAddr)> {
        let Some(peer) = self.peer_addrs.load().as_ref().map(|peers| {
            let v = peers.pick_random();
            *v.inner()
        }) else {
            return Err(anyhow!("no peer address available"));
        };

        self.stats.add_conn_attempt();

        let sock = g3_socket::tcp::new_socket_to(
            peer.ip(),
            &Default::default(),
            &self.config.tcp_keepalive,
            &Default::default(),
            true,
        )?;

        let stream = sock
            .connect(peer)
            .await
            .map_err(|e| anyhow!("failed to connect to peer {peer}: {e}"))?;
        self.stats.add_conn_established();

        Ok((stream, peer))
    }
}

#[async_trait]
impl KeylessUpstreamConnect for KeylessTcpUpstreamConnector {
    type Connection = MultiplexedUpstreamConnection<tcp::OwnedReadHalf, tcp::OwnedWriteHalf>;

    async fn new_connection(
        &self,
        req_receiver: flume::Receiver<KeylessForwardRequest>,
        quit_notifier: broadcast::Receiver<()>,
    ) -> anyhow::Result<Self::Connection> {
        let start = Instant::now();
        let (stream, _peer) = self.connect().await?;
        let _ = self
            .duration_recorder
            .connect
            .record(start.elapsed().as_nanos_u64());
        let (clt_r, clt_w) = stream.into_split();

        Ok(MultiplexedUpstreamConnection::new(
            self.config.connection_config,
            self.stats.clone(),
            self.duration_recorder.clone(),
            clt_r,
            clt_w,
            req_receiver,
            quit_notifier,
        ))
    }
}

pub(super) struct KeylessTlsUpstreamConnector {
    tcp: KeylessTcpUpstreamConnector,
    tls: RustlsClientConfig,
}

impl KeylessTlsUpstreamConnector {
    pub(super) fn new(tcp: KeylessTcpUpstreamConnector, tls: RustlsClientConfig) -> Self {
        KeylessTlsUpstreamConnector { tcp, tls }
    }
}

#[async_trait]
impl KeylessUpstreamConnect for KeylessTlsUpstreamConnector {
    type Connection = MultiplexedUpstreamConnection<
        ReadHalf<TlsStream<TcpStream>>,
        WriteHalf<TlsStream<TcpStream>>,
    >;

    async fn new_connection(
        &self,
        req_receiver: flume::Receiver<KeylessForwardRequest>,
        quit_notifier: broadcast::Receiver<()>,
    ) -> anyhow::Result<Self::Connection> {
        let start = Instant::now();
        let (tcp_stream, peer) = self.tcp.connect().await?;

        let tls_name = self
            .tcp
            .config
            .tls_name
            .clone()
            .unwrap_or_else(|| ServerName::IpAddress(peer.ip().into()));
        let tls_connector = TlsConnector::from(self.tls.driver.clone());
        match tokio::time::timeout(
            self.tls.handshake_timeout,
            tls_connector.connect(tls_name, tcp_stream),
        )
        .await
        {
            Ok(Ok(tls_stream)) => {
                let _ = self
                    .tcp
                    .duration_recorder
                    .connect
                    .record(start.elapsed().as_nanos_u64());
                let (clt_r, clt_w) = tls_stream.into_split();

                Ok(MultiplexedUpstreamConnection::new(
                    self.tcp.config.connection_config,
                    self.tcp.stats.clone(),
                    self.tcp.duration_recorder.clone(),
                    clt_r,
                    clt_w,
                    req_receiver,
                    quit_notifier,
                ))
            }
            Ok(Err(e)) => Err(anyhow!("tls handshake failed: {e}")),
            Err(_) => Err(anyhow!("tls handshake timeout")),
        }
    }
}
