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

use tokio::io::{AsyncRead, BufReader};
use tokio::sync::{broadcast, oneshot};
use tokio::time::Interval;

use g3_io_ext::LimitedBufReadExt;
use g3_types::ext::DurationExt;

use super::StreamSharedState;
use crate::module::keyless::{
    KeylessBackendStats, KeylessRecvMessageError, KeylessUpstreamDurationRecorder,
    KeylessUpstreamResponse,
};

pub(super) struct KeylessUpstreamRecvTask {
    rsp_recv_timeout: Duration,
    stats: Arc<KeylessBackendStats>,
    quit_notifier: broadcast::Receiver<()>,
    reader_close_sender: oneshot::Sender<KeylessRecvMessageError>,
    shared_state: Arc<StreamSharedState>,
    duration_recorder: Arc<KeylessUpstreamDurationRecorder>,
}

impl KeylessUpstreamRecvTask {
    pub(super) fn new(
        rsp_recv_timeout: Duration,
        stats: Arc<KeylessBackendStats>,
        quit_notifier: broadcast::Receiver<()>,
        close_sender: oneshot::Sender<KeylessRecvMessageError>,
        shared_state: Arc<StreamSharedState>,
        duration_recorder: Arc<KeylessUpstreamDurationRecorder>,
    ) -> Self {
        KeylessUpstreamRecvTask {
            rsp_recv_timeout,
            stats,
            quit_notifier,
            reader_close_sender: close_sender,
            shared_state,
            duration_recorder,
        }
    }

    pub(super) async fn into_running<R>(mut self, reader: R)
    where
        R: AsyncRead + Unpin,
    {
        if let Err(e) = self.run_online(reader).await {
            let _ = self.reader_close_sender.send(e);
        }
        self.shared_state.drain(|_id, v| {
            self.stats.add_request_drop();
            v.send_internal_error();
        });
    }

    async fn run_online<R>(&mut self, reader: R) -> Result<(), KeylessRecvMessageError>
    where
        R: AsyncRead + Unpin,
    {
        let mut timeout_interval = tokio::time::interval(self.rsp_recv_timeout);
        let mut buf_reader = BufReader::new(reader);
        loop {
            tokio::select! {
                biased;

                r = buf_reader.fill_wait_data() => {
                    match r {
                        Ok(true) => {
                            let rsp = KeylessUpstreamResponse::recv(&mut buf_reader).await?;
                            self.handle_rsp(rsp);
                        }
                        Ok(false) => return Err(KeylessRecvMessageError::IoClosed),
                        Err(e) => return Err(KeylessRecvMessageError::IoFailed(e)),
                    }
                }
                _ = self.quit_notifier.recv() => {
                    return tokio::time::timeout(self.rsp_recv_timeout, self.recv_all_pending(buf_reader))
                        .await
                        .unwrap_or(Ok(()));
                }
                _ = self.reader_close_sender.closed() => {
                    return self.run_offline(buf_reader, timeout_interval).await;
                }
                _ = timeout_interval.tick() => {
                    self.shared_state.rotate_timeout(|_id, v| {
                        self.stats.add_request_drop();
                        v.send_internal_error();
                    });
                }
            }
        }
    }

    async fn run_offline<R>(
        &mut self,
        mut buf_reader: BufReader<R>,
        mut timeout_interval: Interval,
    ) -> Result<(), KeylessRecvMessageError>
    where
        R: AsyncRead + Unpin,
    {
        loop {
            if !self.shared_state.has_pending() {
                return Ok(());
            }

            tokio::select! {
                biased;

                r = buf_reader.fill_wait_data() => {
                    match r {
                        Ok(true) => {
                            let rsp = KeylessUpstreamResponse::recv(&mut buf_reader).await?;
                            self.handle_rsp(rsp);
                        }
                        Ok(false) => return Err(KeylessRecvMessageError::IoClosed),
                        Err(e) => return Err(KeylessRecvMessageError::IoFailed(e)),
                    }
                }
                _ = self.quit_notifier.recv() => {
                    return tokio::time::timeout(self.rsp_recv_timeout, self.recv_all_pending(buf_reader))
                        .await
                        .unwrap_or(Ok(()));
                }
                _ = timeout_interval.tick() => {
                    self.shared_state.rotate_timeout(|_id, v| {
                        self.stats.add_request_drop();
                        v.send_internal_error();
                    });
                }
            }
        }
    }

    async fn recv_all_pending<R>(&self, mut reader: R) -> Result<(), KeylessRecvMessageError>
    where
        R: AsyncRead + Unpin,
    {
        loop {
            if !self.shared_state.has_pending() {
                return Ok(());
            }
            let rsp = KeylessUpstreamResponse::recv(&mut reader).await?;
            self.handle_rsp(rsp);
        }
    }

    fn handle_rsp(&self, rsp: KeylessUpstreamResponse) {
        let rsp_id = rsp.id();
        self.stats.add_response_recv();
        if let Some(v) = self.shared_state.fetch_request(rsp_id) {
            let _ = self
                .duration_recorder
                .response
                .record(v.elapsed().as_nanos_u64());
            match v.send_upstream_rsp(rsp) {
                Ok(_) => self.stats.add_response_send(),
                Err(_) => self.stats.add_response_drop(),
            }
        } else {
            self.stats.add_response_drop();
        }
    }
}
