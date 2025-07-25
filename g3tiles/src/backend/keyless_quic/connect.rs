/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::borrow::Cow;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use quinn::{ClientConfig, Connection, Endpoint, TokioRuntime, TransportConfig};
use tokio::sync::broadcast;
use tokio::time::Instant;

use g3_std_ext::time::DurationExt;
use g3_types::collection::{SelectiveVec, WeightedValue};
use g3_types::net::RustlsQuicClientConfig;

use crate::config::backend::keyless_quic::KeylessQuicBackendConfig;
use crate::module::keyless::{
    KeylessBackendStats, KeylessForwardRequest, KeylessUpstreamConnect, KeylessUpstreamConnection,
    KeylessUpstreamDurationRecorder, MultiplexedUpstreamConnection,
};

pub(super) struct KeylessQuicUpstreamConnector {
    config: Arc<KeylessQuicBackendConfig>,
    stats: Arc<KeylessBackendStats>,
    duration_recorder: Arc<KeylessUpstreamDurationRecorder>,
    peer_addrs: Arc<ArcSwapOption<SelectiveVec<WeightedValue<SocketAddr>>>>,
    tls_client: RustlsQuicClientConfig,
    quic_transport: Arc<TransportConfig>,
}

impl KeylessQuicUpstreamConnector {
    pub(super) fn new(
        config: Arc<KeylessQuicBackendConfig>,
        stats: Arc<KeylessBackendStats>,
        duration_recorder: Arc<KeylessUpstreamDurationRecorder>,
        peer_addrs_container: Arc<ArcSwapOption<SelectiveVec<WeightedValue<SocketAddr>>>>,
    ) -> anyhow::Result<Self> {
        let tls_client = config.tls_client.build_quic()?;
        let quic_transport = config.quic_transport.build_for_client();
        Ok(KeylessQuicUpstreamConnector {
            config,
            stats,
            duration_recorder,
            peer_addrs: peer_addrs_container,
            tls_client,
            quic_transport: Arc::new(quic_transport),
        })
    }

    async fn connect(&self) -> anyhow::Result<Connection> {
        let Some(peer) = self.peer_addrs.load().as_ref().map(|peers| {
            let v = peers.pick_random();
            *v.inner()
        }) else {
            return Err(anyhow!("no peer address available"));
        };

        self.stats.add_conn_attempt();

        let socket = g3_socket::udp::new_std_socket_to(
            peer,
            &Default::default(),
            self.config.socket_buffer,
            Default::default(),
        )
        .map_err(|e| anyhow!("failed to setup local udp socket: {e}"))?;
        socket
            .connect(peer)
            .map_err(|e| anyhow!("failed to connect local udp socket to {peer}: {e}"))?;

        let endpoint = Endpoint::new(Default::default(), None, socket, Arc::new(TokioRuntime))
            .map_err(|e| anyhow!("failed to create quic endpoint: {e}"))?;

        let mut client_config = ClientConfig::new(self.tls_client.driver.clone());
        client_config.transport_config(self.quic_transport.clone());
        let tls_name = self
            .config
            .tls_name
            .as_ref()
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(peer.ip().to_string()));
        let client_connect = endpoint
            .connect_with(client_config, peer, &tls_name)
            .map_err(|e| anyhow!("failed to create quic client: {e}"))?;

        let conn = tokio::time::timeout(self.tls_client.handshake_timeout, client_connect)
            .await
            .map_err(|_| anyhow!("quic connect to peer {peer} time out"))?
            .map_err(|e| anyhow!("quic connect to peer {peer} failed: {e}"))?;
        self.stats.add_conn_established();

        Ok(conn)
    }
}

#[async_trait]
impl KeylessUpstreamConnect for KeylessQuicUpstreamConnector {
    type Connection = KeylessQuicUpstreamConnection;

    async fn new_connection(
        &self,
        req_receiver: flume::Receiver<KeylessForwardRequest>,
        quit_notifier: broadcast::Receiver<()>,
        idle_timeout: Duration,
    ) -> anyhow::Result<Self::Connection> {
        let start = Instant::now();
        let conn = self.connect().await?;
        let _ = self
            .duration_recorder
            .connect
            .record(start.elapsed().as_nanos_u64());

        for _ in 0..self.config.concurrent_streams {
            let Ok((send_stream, recv_stream)) = conn.open_bi().await else {
                break;
            };

            let connection = MultiplexedUpstreamConnection::new(
                self.config.connection_config,
                self.stats.clone(),
                self.duration_recorder.clone(),
                recv_stream,
                send_stream,
                req_receiver.clone(),
                quit_notifier.resubscribe(),
            );
            tokio::spawn(async move {
                let _ = connection.run(idle_timeout).await;
            });
        }

        Ok(KeylessQuicUpstreamConnection {
            c: conn,
            quit_notifier,
        })
    }
}

pub(crate) struct KeylessQuicUpstreamConnection {
    c: Connection,
    quit_notifier: broadcast::Receiver<()>,
}

impl KeylessUpstreamConnection for KeylessQuicUpstreamConnection {
    async fn run(mut self, _idle_timeout: Duration) -> anyhow::Result<()> {
        tokio::select! {
            e = self.c.closed() => {
                Err(anyhow::Error::new(e))
            }
            _ = self.quit_notifier.recv() => {
                self.c.closed().await;
                Ok(())
            }
        }
    }
}
