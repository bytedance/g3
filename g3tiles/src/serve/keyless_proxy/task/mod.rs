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

use std::sync::Arc;
use std::time::Duration;

use log::debug;
use tokio::io::{AsyncRead, AsyncWrite, BufReader};
use tokio::sync::mpsc;
use tokio::time::Instant;

use g3_io_ext::LimitedBufReadExt;

use crate::config::server::ServerConfig;
use crate::log::task::keyless::TaskLogForKeyless;
use crate::module::keyless::{KeylessRequest, KeylessResponse};
use crate::serve::{ServerTaskError, ServerTaskNotes, ServerTaskResult};

mod common;
pub(super) use common::CommonTaskContext;

mod stats;
use stats::KeylessTaskStats;

pub(super) struct KeylessForwardTask {
    ctx: CommonTaskContext,
    stats: Arc<KeylessTaskStats>,
    task_notes: ServerTaskNotes,
}

impl KeylessForwardTask {
    pub(super) fn new(ctx: CommonTaskContext) -> Self {
        let task_notes = ServerTaskNotes::new(ctx.cc_info.clone(), Duration::ZERO);
        KeylessForwardTask {
            ctx,
            stats: Arc::new(KeylessTaskStats::default()),
            task_notes,
        }
    }

    fn get_log_context(&self) -> TaskLogForKeyless {
        TaskLogForKeyless {
            task_notes: &self.task_notes,
            task_stats: self.stats.relay.snapshot(),
        }
    }

    pub(super) async fn into_running<R, W>(mut self, clt_r: R, clt_w: W)
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        self.pre_start();

        if let Err(e) = self.run(clt_r, clt_w).await {
            self.get_log_context().log(&self.ctx.task_logger, &e);
        }

        self.pre_stop();
    }

    fn pre_start(&self) {
        debug!(
            "KeylessProxy: new client from {} to {} server {}",
            self.ctx.client_addr(),
            self.ctx.server_config.server_type(),
            self.ctx.server_config.name(),
        );
        self.ctx.server_stats.add_task();
        self.ctx.server_stats.inc_alive_task();
    }

    fn pre_stop(&self) {
        self.ctx.server_stats.dec_alive_task();
    }

    pub(super) async fn run<R, W>(&mut self, clt_r: R, mut clt_w: W) -> ServerTaskResult<()>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let (rsp_sender, mut rsp_receiver) = mpsc::channel::<KeylessResponse>(512); // TODO
        let server_stats = self.ctx.server_stats.clone();
        let task_stats = self.stats.clone();
        let send_task = tokio::spawn(async move {
            // TODO use batch recv
            while let Some(rsp) = rsp_receiver.recv().await {
                match rsp.send(&mut clt_w).await {
                    Ok(_) => {
                        server_stats.relay.add_rsp_pass();
                        task_stats.relay.add_rsp_pass();
                        task_stats.mark_active();
                    }
                    Err(_e) => {
                        // TODO log error ?
                        server_stats.relay.add_rsp_fail();
                        task_stats.relay.add_rsp_fail();
                        break;
                    }
                }
            }
            while let Some(_rsp) = rsp_receiver.recv().await {
                server_stats.relay.add_rsp_drop();
                task_stats.relay.add_rsp_drop();
            }
        });

        self.task_notes.mark_relaying();
        let r = self.run_recv(clt_r, rsp_sender).await;
        let _ = send_task.await;
        r
    }

    async fn run_recv<R>(
        &self,
        clt_r: R,
        rsp_sender: mpsc::Sender<KeylessResponse>,
    ) -> ServerTaskResult<()>
    where
        R: AsyncRead + Unpin,
    {
        let idle_duration = self.ctx.server_config.task_idle_check_duration;
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;

        let mut buf_reader = BufReader::new(clt_r);
        loop {
            tokio::select! {
                biased;

                r = buf_reader.fill_wait_data() => {
                    match r {
                        Ok(true) => {
                            self.recv_request(&mut buf_reader, &rsp_sender).await?;
                        }
                        Ok(false) => return Err(ServerTaskError::ClosedByClient),
                        Err(e) => return Err(ServerTaskError::ClientTcpReadFailed(e)),
                    }
                }
                _ = idle_interval.tick() => {
                    if self.stats.check_idle() {
                        idle_count += 1;

                        if idle_count >= self.ctx.server_config.task_idle_max_count {
                            return Err(ServerTaskError::Idle(idle_duration, idle_count));
                        }
                    } else {
                        idle_count = 0;
                    }

                    if self.ctx.server_quit_policy.force_quit() {
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            }
        }
    }

    async fn recv_request<R>(
        &self,
        clt_r: &mut R,
        rsp_sender: &mpsc::Sender<KeylessResponse>,
    ) -> ServerTaskResult<()>
    where
        R: AsyncRead + Unpin,
    {
        let req = KeylessRequest::recv(clt_r).await?;
        self.ctx.server_stats.relay.add_req_total();
        self.stats.relay.add_req_total();
        self.stats.mark_active();

        let backend = self.ctx.select_backend();
        let server_stats = self.ctx.server_stats.clone();
        let task_stats = self.stats.clone();
        let rsp_sender = rsp_sender.clone();
        tokio::spawn(async move {
            let rsp = backend.keyless(req).await;
            match rsp {
                KeylessResponse::Upstream(_) => {
                    server_stats.relay.add_req_pass();
                    task_stats.relay.add_req_pass();
                }
                KeylessResponse::Local(_) => {
                    server_stats.relay.add_req_fail();
                    task_stats.relay.add_req_fail();
                }
            }
            if rsp_sender.send(rsp).await.is_err() {
                server_stats.relay.add_rsp_drop();
                task_stats.relay.add_rsp_drop();
            }
        });

        Ok(())
    }
}
