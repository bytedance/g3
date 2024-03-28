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

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{broadcast, oneshot};

use crate::module::keyless::{KeylessBackendStats, KeylessUpstreamDurationRecorder};

mod state;
use state::StreamSharedState;

mod recv;
use recv::KeylessUpstreamRecvTask;

mod send;
use send::KeylessUpstreamSendTask;

use super::{KeylessForwardRequest, KeylessUpstreamConnection};

pub(crate) struct MultiplexedUpstreamConnection<R, W> {
    rsp_timeout: Duration,
    stats: Arc<KeylessBackendStats>,
    duration_recorder: Arc<KeylessUpstreamDurationRecorder>,
    r: R,
    w: W,
    req_receiver: flume::Receiver<KeylessForwardRequest>,
    quit_notifier: broadcast::Receiver<Duration>,
}

impl<R, W> MultiplexedUpstreamConnection<R, W> {
    pub(crate) fn new(
        rsp_timeout: Duration,
        stats: Arc<KeylessBackendStats>,
        duration_recorder: Arc<KeylessUpstreamDurationRecorder>,
        ups_r: R,
        ups_w: W,
        req_receiver: flume::Receiver<KeylessForwardRequest>,
        quit_notifier: broadcast::Receiver<Duration>,
    ) -> Self {
        MultiplexedUpstreamConnection {
            rsp_timeout,
            stats,
            duration_recorder,
            r: ups_r,
            w: ups_w,
            req_receiver,
            quit_notifier,
        }
    }
}

impl<R, W> KeylessUpstreamConnection for MultiplexedUpstreamConnection<R, W>
where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin,
{
    async fn run(self) -> anyhow::Result<()> {
        let shared_state = Arc::new(StreamSharedState::default());
        let (reader_close_sender, reader_close_receiver) = oneshot::channel();

        let send_task = KeylessUpstreamSendTask::new(
            self.stats.clone(),
            self.req_receiver,
            self.quit_notifier.resubscribe(),
            reader_close_receiver,
            shared_state.clone(),
            self.duration_recorder.clone(),
        );
        let recv_task = KeylessUpstreamRecvTask::new(
            self.rsp_timeout,
            self.stats,
            self.quit_notifier,
            reader_close_sender,
            shared_state,
            self.duration_recorder,
        );

        let reader = self.r;
        tokio::spawn(async move { recv_task.into_running(reader).await });
        send_task.run(self.w).await
    }
}
