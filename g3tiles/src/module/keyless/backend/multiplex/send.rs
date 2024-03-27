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

use std::io;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use tokio::io::AsyncWrite;
use tokio::sync::{broadcast, oneshot};

use super::StreamSharedState;
use crate::module::keyless::{KeylessBackendStats, KeylessForwardRequest, KeylessRecvMessageError};

pub(super) struct KeylessUpstreamSendTask {
    stats: Arc<KeylessBackendStats>,
    req_receiver: flume::Receiver<KeylessForwardRequest>,
    quit_notifier: broadcast::Receiver<Duration>,
    reader_close_receiver: oneshot::Receiver<KeylessRecvMessageError>,
    shared_state: Arc<StreamSharedState>,
    message_id: u32,
}

impl KeylessUpstreamSendTask {
    pub(super) fn new(
        stats: Arc<KeylessBackendStats>,
        req_receiver: flume::Receiver<KeylessForwardRequest>,
        quit_notifier: broadcast::Receiver<Duration>,
        reader_close_receiver: oneshot::Receiver<KeylessRecvMessageError>,
        shared_state: Arc<StreamSharedState>,
    ) -> Self {
        KeylessUpstreamSendTask {
            stats,
            req_receiver,
            quit_notifier,
            reader_close_receiver,
            shared_state,
            message_id: 0,
        }
    }

    pub(super) async fn run<W>(mut self, mut writer: W) -> anyhow::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        if let Some(wait) = self.run_online(&mut writer).await? {
            tokio::time::sleep(wait).await;
        }
        Ok(())
    }

    async fn run_online<W>(&mut self, writer: &mut W) -> anyhow::Result<Option<Duration>>
    where
        W: AsyncWrite + Unpin,
    {
        loop {
            tokio::select! {
                biased;

                r = self.req_receiver.recv_async() => {
                    match r {
                        Ok(req) => {
                            self.send_data(writer, req)
                                .await
                                .map_err(|e| anyhow!("send request failed: {e}"))?;
                        }
                        Err(_) => return Err(anyhow!("backend dropped")),
                    }
                }
                r = self.quit_notifier.recv() => {
                    return match r {
                        Ok(wait) => Ok(Some(wait)),
                        Err(_) => Err(anyhow!("pool dropped")),
                    };
                }
                r = &mut self.reader_close_receiver => {
                    return match r {
                        Ok(e) => Err(anyhow!("reader side closed with error {e}")),
                        Err(_) => Ok(None), // reader closed without error
                    };
                }
            }
        }
    }

    async fn send_data<W>(&mut self, writer: &mut W, data: KeylessForwardRequest) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
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
