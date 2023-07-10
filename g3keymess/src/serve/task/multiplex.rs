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

use std::io;

use anyhow::anyhow;
use log::warn;
use openssl::pkey::{PKey, Private};
use openssl_async_job::{SyncOperation, TokioAsyncOperation};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, mpsc};

use g3_io_ext::LimitedBufReadExt;

use super::KeylessTask;
use crate::protocol::{KeylessErrorResponse, KeylessRequest, KeylessResponse};
use crate::serve::ServerReloadCommand;

impl KeylessTask {
    pub(crate) async fn into_multiplex_running<R, W>(mut self, reader: R, mut writer: W)
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let (msg_sender, mut msg_receiver) =
            mpsc::channel::<KeylessResponse>(self.ctx.server_config.multiplex_queue_depth);

        let write_handle = tokio::spawn(async move {
            let mut write_error: io::Result<()> = Ok(());
            'outer: while let Some(rsp) = msg_receiver.recv().await {
                if let Err(e) = writer.write_all(rsp.message()).await {
                    write_error = Err(e);
                    break;
                }

                while let Ok(rsp) = msg_receiver.try_recv() {
                    if let Err(e) = writer.write_all(rsp.message()).await {
                        write_error = Err(e);
                        break 'outer;
                    }
                }

                if let Err(e) = writer.flush().await {
                    write_error = Err(e);
                    break;
                }
            }
            msg_receiver.close();
            write_error
        });

        let _read_result = self.read_till_end(reader, &msg_sender).await;
        drop(msg_sender);

        let _write_result = write_handle.await;
    }

    async fn read_till_end<R>(
        &mut self,
        reader: R,
        msg_sender: &mpsc::Sender<KeylessResponse>,
    ) -> anyhow::Result<()>
    where
        R: AsyncRead + Send + Unpin + 'static,
    {
        let mut buf_reader = BufReader::new(reader);

        loop {
            tokio::select! {
                biased;

                r = buf_reader.fill_wait_data() => {
                    match r {
                        Ok(true) => {
                            if let Err(e) = self.read_and_spawn(&mut buf_reader, msg_sender).await {
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

        Ok(())
    }

    async fn read_and_spawn<R>(
        &mut self,
        reader: &mut R,
        msg_sender: &mpsc::Sender<KeylessResponse>,
    ) -> anyhow::Result<()>
    where
        R: AsyncRead + Send + Unpin + 'static,
    {
        let req = self.timed_read_request(reader).await?;
        if let Some(pong) = req.ping_pong() {
            return msg_sender
                .send(KeylessResponse::Pong(pong))
                .await
                .map_err(|e| anyhow!("writer closed while send pong response {}", e.0.id()));
        }

        let rsp = KeylessErrorResponse::new(req.id);

        let Some(key) = req.find_key() else {
            return msg_sender.send(KeylessResponse::Error(rsp.key_not_found())).await
                .map_err(|e| anyhow!("writer closed while send error response {}", e.0.id()));
        };

        self.async_process_by_openssl(req, rsp, key, msg_sender)
            .await
    }

    async fn async_process_by_openssl(
        &self,
        req: KeylessRequest,
        rsp: KeylessErrorResponse,
        key: PKey<Private>,
        msg_sender: &mpsc::Sender<KeylessResponse>,
    ) -> anyhow::Result<()> {
        let server_sem = if let Some(sem) = self.ctx.concurrency_limit.clone() {
            sem.acquire_owned().await.ok()
        } else {
            None
        };

        let sync_op = OpensslOperation { req, key };
        let Ok(task) = TokioAsyncOperation::build_async_task(sync_op) else {
            return msg_sender.send(KeylessResponse::Error(rsp.crypto_fail())).await
                .map_err(|e| anyhow!("writer closed while send error response {}", e.0.id()));
        };

        let msg_sender = msg_sender.clone();
        let async_op_timeout = self.ctx.server_config.async_op_timeout;
        tokio::spawn(async move {
            let rsp = match tokio::time::timeout(async_op_timeout, task).await {
                Ok(Ok(r)) => r,
                Ok(Err(_)) => KeylessResponse::Error(rsp.crypto_fail()),
                Err(_) => KeylessResponse::Error(rsp.crypto_fail()),
            };
            drop(server_sem);
            // send to writer
            if let Err(e) = msg_sender.send(rsp).await {
                warn!("writer closed while send response {}", e.0.id());
            }
        });
        Ok(())
    }
}

struct OpensslOperation {
    req: KeylessRequest,
    key: PKey<Private>,
}

impl SyncOperation for OpensslOperation {
    type Output = KeylessResponse;

    fn run(&mut self) -> anyhow::Result<Self::Output> {
        let rsp = match self.req.process(&self.key) {
            Ok(d) => KeylessResponse::Data(d),
            Err(e) => KeylessResponse::Error(e),
        };
        Ok(rsp)
    }
}
