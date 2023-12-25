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

use openssl::pkey::{PKey, Private};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, mpsc};

use g3_io_ext::LimitedBufReadExt;
use g3_openssl::async_job::{SyncOperation, TokioAsyncOperation};
use g3_types::ext::DurationExt;

use super::{KeylessTask, WrappedKeylessRequest};
use crate::log::request::RequestErrorLogContext;
use crate::protocol::{KeylessErrorResponse, KeylessResponse};
use crate::serve::{ServerReloadCommand, ServerTaskError};

impl KeylessTask {
    pub(crate) async fn into_multiplex_running<R, W>(mut self, reader: R, mut writer: W)
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let (msg_sender, mut msg_receiver) =
            mpsc::channel::<KeylessResponse>(self.ctx.server_config.multiplex_queue_depth);

        let task_id = self.id;
        let request_logger = self.ctx.request_logger.clone();
        let write_handle = tokio::spawn(async move {
            let mut write_error: Result<(), ServerTaskError> = Ok(());

            let request_log_ctx = RequestErrorLogContext { task_id: &task_id };

            'outer: while let Some(rsp) = msg_receiver.recv().await {
                request_log_ctx.log(&request_logger, &rsp);
                if let Err(e) = writer.write_all(rsp.message()).await {
                    write_error = Err(ServerTaskError::WriteFailed(e));
                    break;
                }

                while let Ok(rsp) = msg_receiver.try_recv() {
                    request_log_ctx.log(&request_logger, &rsp);
                    if let Err(e) = writer.write_all(rsp.message()).await {
                        write_error = Err(ServerTaskError::WriteFailed(e));
                        break 'outer;
                    }
                }

                if let Err(e) = writer.flush().await {
                    write_error = Err(ServerTaskError::WriteFailed(e));
                    break;
                }
            }
            msg_receiver.close();
            write_error
        });

        let mut log_ok = true;
        if let Err(e) = self.read_till_end(reader, &msg_sender).await {
            self.log_task_err(e);
            log_ok = false;
        }

        drop(msg_sender);
        match write_handle.await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                self.log_task_err(e);
                return;
            }
            Err(_) => {}
        }

        if log_ok {
            self.log_task_ok();
        }
    }

    async fn read_till_end<R>(
        &mut self,
        reader: R,
        msg_sender: &mpsc::Sender<KeylessResponse>,
    ) -> Result<(), ServerTaskError>
    where
        R: AsyncRead + Send + Unpin + 'static,
    {
        let mut buf_reader = BufReader::new(reader);
        let mut msg_count = 0;

        loop {
            tokio::select! {
                biased;

                r = buf_reader.fill_wait_data() => {
                    match r {
                        Ok(true) => {
                            self.read_and_spawn(&mut buf_reader, msg_count, msg_sender).await?;
                            msg_count += 1;
                        }
                        Ok(false) => return Ok(()),
                        Err(e) => {
                            return if msg_count == 0 {
                                Err(ServerTaskError::ConnectionClosedEarly)
                            } else {
                                Err(ServerTaskError::ReadFailed(e))
                            };
                        }
                    }
                }
                r = self.ctx.reload_notifier.recv() => {
                    match r {
                        Ok(ServerReloadCommand::QuitRuntime) => {
                            // TODO close connection gracefully
                            return Err(ServerTaskError::ServerForceQuit);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // force quit
                            return Err(ServerTaskError::ServerForceQuit);
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                    }
                }
            }
        }
    }

    async fn read_and_spawn<R>(
        &mut self,
        reader: &mut R,
        msg_count: usize,
        msg_sender: &mpsc::Sender<KeylessResponse>,
    ) -> Result<(), ServerTaskError>
    where
        R: AsyncRead + Send + Unpin + 'static,
    {
        let mut req = self.timed_read_request(reader, msg_count).await?;
        if let Some(rsp) = req.take_err_rsp() {
            req.stats.add_by_error_code(rsp.error_code());
            let _ = msg_sender.send(KeylessResponse::Error(rsp)).await;
            return Ok(());
        }

        if let Some(pong) = req.inner.ping_pong() {
            req.stats.add_passed();
            let _ = msg_sender.send(KeylessResponse::Pong(pong)).await;
            return Ok(());
        }

        let key = match req.inner.find_key() {
            Ok(key) => key,
            Err(rsp) => {
                req.stats.add_by_error_code(rsp.error_code());
                let _ = msg_sender.send(KeylessResponse::Error(rsp)).await;
                return Ok(());
            }
        };

        let rsp = KeylessErrorResponse::new(req.inner.id);
        self.async_process_by_openssl(req, rsp, key, msg_sender)
            .await;
        Ok(())
    }

    async fn async_process_by_openssl(
        &self,
        req: WrappedKeylessRequest,
        rsp: KeylessErrorResponse,
        key: PKey<Private>,
        msg_sender: &mpsc::Sender<KeylessResponse>,
    ) {
        let server_sem = if let Some(sem) = self.ctx.concurrency_limit.clone() {
            sem.acquire_owned().await.ok()
        } else {
            None
        };

        let create_time = req.create_time;
        let duration_recorder = req.duration_recorder.clone();
        let req_stats = req.stats.clone();
        let sync_op = OpensslOperation { req, key };
        let Ok(task) = TokioAsyncOperation::build_async_task(sync_op) else {
            req_stats.add_crypto_fail();
            let _ = msg_sender
                .send(KeylessResponse::Error(rsp.crypto_fail()))
                .await;
            return;
        };

        let msg_sender = msg_sender.clone();
        let async_op_timeout = self.ctx.server_config.async_op_timeout;
        tokio::spawn(async move {
            let rsp = match tokio::time::timeout(async_op_timeout, task).await {
                Ok(Ok(r)) => {
                    req_stats.add_passed();
                    r
                }
                Ok(Err(_)) => {
                    req_stats.add_crypto_fail();
                    KeylessResponse::Error(rsp.crypto_fail())
                }
                Err(_) => {
                    req_stats.add_crypto_fail();
                    KeylessResponse::Error(rsp.crypto_fail())
                }
            };
            drop(server_sem);
            // send to writer
            let _ = msg_sender.send(rsp).await;
            let _ = duration_recorder.record(create_time.elapsed().as_nanos_u64());
        });
    }
}

struct OpensslOperation {
    req: WrappedKeylessRequest,
    key: PKey<Private>,
}

impl SyncOperation for OpensslOperation {
    type Output = KeylessResponse;

    fn run(&mut self) -> anyhow::Result<Self::Output> {
        let rsp = match self.req.inner.process(&self.key) {
            Ok(d) => KeylessResponse::Data(d),
            Err(e) => KeylessResponse::Error(e),
        };
        Ok(rsp)
    }
}
