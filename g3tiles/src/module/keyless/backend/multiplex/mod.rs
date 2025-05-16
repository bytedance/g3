/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{broadcast, oneshot};

use super::{KeylessForwardRequest, KeylessUpstreamConnection};
use crate::module::keyless::{
    KeylessBackendAliveChannelGuard, KeylessBackendStats, KeylessUpstreamDurationRecorder,
};

mod state;
use state::StreamSharedState;

mod recv;
use recv::KeylessUpstreamRecvTask;

mod send;
use send::KeylessUpstreamSendTask;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MultiplexedUpstreamConnectionConfig {
    pub(crate) max_request_count: usize,
    pub(crate) max_alive_time: Duration,
    pub(crate) response_timeout: Duration,
}

impl Default for MultiplexedUpstreamConnectionConfig {
    fn default() -> Self {
        MultiplexedUpstreamConnectionConfig {
            max_request_count: 4000,
            max_alive_time: Duration::from_secs(3600), // 1h
            response_timeout: Duration::from_secs(4),
        }
    }
}

pub(crate) struct MultiplexedUpstreamConnection<R, W> {
    config: MultiplexedUpstreamConnectionConfig,
    stats: Arc<KeylessBackendStats>,
    duration_recorder: Arc<KeylessUpstreamDurationRecorder>,
    r: R,
    w: W,
    req_receiver: flume::Receiver<KeylessForwardRequest>,
    quit_notifier: broadcast::Receiver<()>,
    alive_channel_guard: KeylessBackendAliveChannelGuard,
}

impl<R, W> MultiplexedUpstreamConnection<R, W> {
    pub(crate) fn new(
        config: MultiplexedUpstreamConnectionConfig,
        stats: Arc<KeylessBackendStats>,
        duration_recorder: Arc<KeylessUpstreamDurationRecorder>,
        ups_r: R,
        ups_w: W,
        req_receiver: flume::Receiver<KeylessForwardRequest>,
        quit_notifier: broadcast::Receiver<()>,
    ) -> Self {
        let alive_channel_guard = stats.inc_alive_channel();
        MultiplexedUpstreamConnection {
            config,
            stats,
            duration_recorder,
            r: ups_r,
            w: ups_w,
            req_receiver,
            quit_notifier,
            alive_channel_guard,
        }
    }
}

impl<R, W> KeylessUpstreamConnection for MultiplexedUpstreamConnection<R, W>
where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin,
{
    async fn run(self, idle_timeout: Duration) -> anyhow::Result<()> {
        let shared_state = Arc::new(StreamSharedState::default());
        let (reader_close_sender, reader_close_receiver) = oneshot::channel();

        let send_task = KeylessUpstreamSendTask::new(
            self.config.max_request_count,
            self.config.max_alive_time,
            self.stats.clone(),
            self.req_receiver,
            self.quit_notifier.resubscribe(),
            shared_state.clone(),
            self.duration_recorder.clone(),
        );
        let recv_task = KeylessUpstreamRecvTask::new(
            self.config.response_timeout,
            self.stats.clone(),
            self.quit_notifier,
            reader_close_sender,
            shared_state,
            self.duration_recorder,
        );

        let reader = self.r;
        let alive_channel_guard = self.alive_channel_guard;
        tokio::spawn(async move {
            recv_task.into_running(reader).await;
            // Only consider the channel off if recv closed.
            drop(alive_channel_guard);
        });
        // The connection is considered off if we no longer need to send request over it,
        // but there may be pending responses on the wire, so let's quit early here to let
        // the connection pool to create new connections early.
        send_task
            .run(self.w, reader_close_receiver, idle_timeout)
            .await
    }
}
