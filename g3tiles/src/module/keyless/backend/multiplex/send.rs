/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::{broadcast, oneshot};
use tokio::time::Instant;

use g3_std_ext::time::DurationExt;

use super::StreamSharedState;
use crate::module::keyless::{
    KeylessBackendStats, KeylessForwardRequest, KeylessRecvMessageError,
    KeylessUpstreamDurationRecorder,
};

pub(super) struct KeylessUpstreamSendTask {
    max_request_count: usize,
    max_alive_time: Duration,
    stats: Arc<KeylessBackendStats>,
    req_receiver: kanal::AsyncReceiver<KeylessForwardRequest>,
    quit_notifier: broadcast::Receiver<()>,
    shared_state: Arc<StreamSharedState>,
    duration_recorder: Arc<KeylessUpstreamDurationRecorder>,
    message_id: u32,
}

impl KeylessUpstreamSendTask {
    pub(super) fn new(
        max_request_count: usize,
        max_alive_time: Duration,
        stats: Arc<KeylessBackendStats>,
        req_receiver: kanal::AsyncReceiver<KeylessForwardRequest>,
        quit_notifier: broadcast::Receiver<()>,
        shared_state: Arc<StreamSharedState>,
        duration_recorder: Arc<KeylessUpstreamDurationRecorder>,
    ) -> Self {
        KeylessUpstreamSendTask {
            max_request_count,
            max_alive_time,
            stats,
            req_receiver,
            quit_notifier,
            shared_state,
            duration_recorder,
            message_id: 0,
        }
    }

    pub(super) async fn run<W>(
        mut self,
        mut writer: W,
        mut reader_close_receiver: oneshot::Receiver<KeylessRecvMessageError>,
        idle_timeout: Duration,
    ) -> anyhow::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut request_count = 0;
        let mut alive_sleep = Box::pin(tokio::time::sleep(self.max_alive_time));
        let mut idle_sleep = Box::pin(tokio::time::sleep(idle_timeout));

        loop {
            tokio::select! {
                biased;

                r = self.req_receiver.recv() => {
                    idle_sleep.as_mut().reset(Instant::now() + idle_timeout);
                    match r {
                        Ok(req) => {
                            request_count += 1;
                            self.send_data(&mut writer, req)
                                .await
                                .map_err(|e| anyhow!("send request failed: {e}"))?;
                            if request_count > self.max_request_count {
                                let _ = writer.shutdown().await;
                                return Ok(());
                            }
                        }
                        Err(_) => {
                            let _ = writer.shutdown().await;
                            return Err(anyhow!("backend dropped"));
                        }
                    }
                }
                _ = self.quit_notifier.recv() => {
                    let _ = writer.shutdown().await;
                    return Ok(());
                }
                r = &mut reader_close_receiver => {
                    return match r {
                        Ok(e) => Err(anyhow!("reader side closed with error {e}")),
                        Err(_) => Ok(()), // reader closed without error
                    };
                }
                _ = &mut alive_sleep => {
                    let _ = writer.shutdown().await;
                    return Ok(());
                }
                _ = &mut idle_sleep => {
                    let _ = writer.shutdown().await;
                    return Ok(());
                }
            }
        }
    }

    async fn send_data<W>(&mut self, writer: &mut W, data: KeylessForwardRequest) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let _ = self
            .duration_recorder
            .wait
            .record(data.created.elapsed().as_nanos_u64());

        let orig_hdr = data.req.header();
        let req = data.req.refresh(self.message_id);
        let rsp_sender = data.rsp_sender;

        self.shared_state
            .add_request(self.message_id, orig_hdr, rsp_sender);
        self.stats.add_request_recv();

        match req.send(writer).await {
            Ok(_) => {
                self.stats.add_request_send();
                self.message_id += 1;
                Ok(())
            }
            Err(e) => {
                self.stats.add_request_drop();
                if let Some(v) = self.shared_state.fetch_request(self.message_id) {
                    v.send_internal_error();
                }
                Err(e)
            }
        }
    }
}
