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
use std::time::Duration;

use anyhow::anyhow;
use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use tokio::net::tcp;
use tokio::sync::broadcast;

use g3_types::collection::{SelectiveVec, WeightedValue};

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
}

#[async_trait]
impl KeylessUpstreamConnect for KeylessTcpUpstreamConnector {
    // TODO use impl TRAIT after 1.79
    type Connection = MultiplexedUpstreamConnection<tcp::OwnedReadHalf, tcp::OwnedWriteHalf>;

    async fn new_connection(
        &self,
        req_receiver: flume::Receiver<KeylessForwardRequest>,
        quit_notifier: broadcast::Receiver<Duration>,
    ) -> anyhow::Result<Self::Connection> {
        let Some(peer) = self.peer_addrs.load().as_ref().map(|peers| {
            let v = peers.pick_random();
            *v.inner()
        }) else {
            return Err(anyhow!("no peer address available"));
        };

        self.stats.add_conn_attempt();

        let sock = g3_socket::tcp::new_socket_to(
            peer.ip(),
            None,
            &self.config.tcp_keepalive,
            &Default::default(),
            true,
        )?;

        let stream = sock
            .connect(peer)
            .await
            .map_err(|e| anyhow!("failed to connect to peer {peer}: {e}"))?;
        self.stats.add_conn_established();

        let (clt_r, clt_w) = stream.into_split();

        Ok(MultiplexedUpstreamConnection::new(
            self.config.response_timeout,
            self.stats.clone(),
            self.duration_recorder.clone(),
            clt_r,
            clt_w,
            req_receiver,
            quit_notifier,
        ))
    }
}
