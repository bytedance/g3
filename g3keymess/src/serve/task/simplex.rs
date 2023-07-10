/*
 * Copyright 2023 ByteDance and/or its affiliates.
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

use anyhow::anyhow;
use log::warn;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::broadcast;

use g3_io_ext::LimitedBufReadExt;

use super::KeylessTask;
use crate::protocol::{KeylessErrorResponse, KeylessResponse};
use crate::serve::ServerReloadCommand;

impl KeylessTask {
    pub(crate) async fn into_simplex_running<R, W>(mut self, reader: R, mut writer: W)
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let mut buf_reader = BufReader::new(reader);

        loop {
            tokio::select! {
                biased;

                r = buf_reader.fill_wait_data() => {
                    match r {
                        Ok(true) => {
                            if let Err(e) = self.read_and_handle(&mut buf_reader, &mut writer).await {
                                warn!("failed to recv request: {e}");
                                break;
                            }
                        }
                        Ok(false) => break,
                        Err(e) => {
                            warn!("failed to read new request: {e}");
                            break;
                        }
                    }
                }
                r = self.ctx.reload_notifier.recv() => {
                    match r {
                        Ok(ServerReloadCommand::QuitRuntime) => {
                            // TODO close connection gracefully
                            break;
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // force quit
                            break;
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                    }
                }
            }
        }
    }

    async fn read_and_handle<R, W>(&mut self, reader: &mut R, writer: &mut W) -> anyhow::Result<()>
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let req = self.timed_read_request(reader).await?;
        if let Some(pong) = req.ping_pong() {
            return self
                .send_response(writer, KeylessResponse::Pong(pong))
                .await;
        }

        let rsp = KeylessErrorResponse::new(req.id);

        let Some(key) = req.find_key() else {
            return self.send_response(writer, KeylessResponse::Error(rsp.key_not_found())).await;
        };

        let server_sem = if let Some(sem) = self.ctx.concurrency_limit.clone() {
            sem.acquire_owned().await.ok()
        } else {
            None
        };

        let rsp = match req.process(&key) {
            Ok(d) => KeylessResponse::Data(d),
            Err(e) => KeylessResponse::Error(e),
        };

        drop(server_sem);
        self.send_response(writer, rsp).await
    }

    pub(super) async fn send_response<W>(
        &self,
        writer: &mut W,
        rsp: KeylessResponse,
    ) -> anyhow::Result<()>
    where
        W: AsyncWrite + Send + Unpin + 'static,
    {
        writer
            .write_all(rsp.message())
            .await
            .map_err(|e| anyhow!("write response failed: {e}"))?;
        writer
            .flush()
            .await
            .map_err(|e| anyhow!("write flush failed: {e}"))?;
        Ok(())
    }
}
