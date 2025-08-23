/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use tokio::io::{AsyncRead, AsyncWrite, BufReader};
use tokio::sync::broadcast;

use g3_io_ext::{LimitedBufReadExt, LimitedWriteExt};

use super::KeylessTask;
use crate::log::request::RequestErrorLogContext;
use crate::protocol::KeylessResponse;
use crate::serve::{RequestProcessContext, ServerReloadCommand, ServerTaskError};

impl KeylessTask {
    pub(crate) async fn into_simplex_running<R, W>(mut self, reader: R, mut writer: W)
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let mut buf_reader = BufReader::new(reader);
        let mut msg_count = 0;

        loop {
            tokio::select! {
                biased;

                r = buf_reader.fill_wait_data() => {
                    match r {
                        Ok(true) => {
                            if let Err(e) = self.read_and_handle(&mut buf_reader, &mut writer, msg_count).await {
                                self.log_task_err(e);
                                break;
                            }
                            msg_count += 1;
                        }
                        Ok(false) => {
                            self.log_task_ok();
                            break;
                        }
                        Err(e) => {
                            if msg_count == 0 {
                                self.log_task_err(ServerTaskError::ConnectionClosedEarly);
                            } else {
                                self.log_task_err(ServerTaskError::ReadFailed(e));
                            }
                            break;
                        }
                    }
                }
                r = self.ctx.reload_notifier.recv() => {
                    match r {
                        Ok(ServerReloadCommand::QuitRuntime) => {
                            // TODO close connection gracefully
                            self.log_task_err(ServerTaskError::ServerForceQuit);
                            break;
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // force quit
                            self.log_task_err(ServerTaskError::ServerForceQuit);
                            break;
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                    }
                }
            }
        }
    }

    async fn read_and_handle<R, W>(
        &mut self,
        reader: &mut R,
        writer: &mut W,
        msg_count: usize,
    ) -> Result<(), ServerTaskError>
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let mut req = self.timed_read_request(reader, msg_count).await?;
        if let Some(rsp) = req.take_err_rsp() {
            req.stats.add_by_error_code(rsp.error_code());
            return self
                .send_response(writer, &req.ctx, KeylessResponse::Error(rsp))
                .await;
        }

        if let Some(pong) = req.inner.ping_pong() {
            req.stats.add_passed();
            return self
                .send_response(writer, &req.ctx, KeylessResponse::Pong(pong))
                .await;
        }

        let key = match req.inner.find_key() {
            Ok(key) => key,
            Err(rsp) => {
                req.stats.add_by_error_code(rsp.error_code());
                return self
                    .send_response(writer, &req.ctx, KeylessResponse::Error(rsp))
                    .await;
            }
        };

        let server_sem = if let Some(sem) = self.ctx.concurrency_limit.clone() {
            sem.acquire_owned().await.ok()
        } else {
            None
        };

        let rsp = req.process_by_openssl(&key);

        drop(server_sem);

        req.ctx.record_duration_stats();
        self.send_response(writer, &req.ctx, rsp).await
    }

    pub(super) async fn send_response<W>(
        &self,
        writer: &mut W,
        ctx: &RequestProcessContext,
        rsp: KeylessResponse,
    ) -> Result<(), ServerTaskError>
    where
        W: AsyncWrite + Send + Unpin + 'static,
    {
        if let Some(logger) = &self.ctx.request_logger {
            RequestErrorLogContext { task_id: &self.id }.log(logger, ctx, &rsp);
        }

        writer
            .write_all_flush(rsp.message())
            .await
            .map_err(ServerTaskError::WriteFailed)?;

        Ok(())
    }
}
