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

use anyhow::anyhow;
use log::debug;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::Instant;
use tokio_openssl::SslStream;

use g3_daemon::stat::task::{TcpStreamConnectionStats, TcpStreamTaskStats};
use g3_io_ext::{LimitedCopy, LimitedCopyConfig, LimitedCopyError, LimitedStream};

use super::{CommonTaskContext, OpensslRelayTaskCltWrapperStats};
use crate::config::server::ServerConfig;
use crate::log::task::tcp_connect::TaskLogForTcpConnect;
use crate::serve::openssl_proxy::{OpensslHost, OpensslService};
use crate::serve::{ServerTaskError, ServerTaskNotes, ServerTaskResult, ServerTaskStage};

pub(crate) struct OpensslRelayTask {
    ctx: CommonTaskContext,
    host: Arc<OpensslHost>,
    service: Arc<OpensslService>,
    task_notes: ServerTaskNotes,
    task_stats: Arc<TcpStreamTaskStats>,
}

impl OpensslRelayTask {
    pub(crate) fn new(
        ctx: CommonTaskContext,
        host: Arc<OpensslHost>,
        service: Arc<OpensslService>,
        wait_time: Duration,
        pre_handshake_stats: Arc<TcpStreamConnectionStats>,
    ) -> Self {
        let task_notes = ServerTaskNotes::new(ctx.client_addr, ctx.server_addr, wait_time);
        OpensslRelayTask {
            ctx,
            host,
            service,
            task_notes,
            task_stats: Arc::new(TcpStreamTaskStats::with_clt_stats(*pre_handshake_stats)),
        }
    }

    fn get_log_context(&self) -> TaskLogForTcpConnect {
        TaskLogForTcpConnect {
            task_notes: &self.task_notes,
            total_time: self.task_notes.time_elapsed(),
            client_rd_bytes: self.task_stats.clt.read.get_bytes(),
            client_wr_bytes: self.task_stats.clt.write.get_bytes(),
            remote_rd_bytes: self.task_stats.ups.read.get_bytes(),
            remote_wr_bytes: self.task_stats.ups.write.get_bytes(),
        }
    }

    pub(crate) async fn into_running<S>(mut self, ssl_stream: SslStream<LimitedStream<S>>)
    where
        S: AsyncRead + AsyncWrite,
    {
        self.pre_start();
        if let Err(e) = self.run(ssl_stream).await {
            self.get_log_context().log(&self.ctx.task_logger, &e)
        }
        self.pre_stop();
    }

    fn pre_start(&self) {
        debug!(
            "OpensslProxy: new client from {} to {} server {}",
            self.ctx.client_addr,
            self.ctx.server_config.server_type(),
            self.ctx.server_config.name(),
        );
        self.ctx.server_stats.add_task();
        self.ctx.server_stats.inc_alive_task();
    }

    fn pre_stop(&self) {
        self.ctx.server_stats.dec_alive_task();
    }

    async fn run<S>(&mut self, ssl_stream: SslStream<LimitedStream<S>>) -> ServerTaskResult<()>
    where
        S: AsyncRead + AsyncWrite,
    {
        self.task_notes.stage = ServerTaskStage::Preparing;

        // set client side socket options
        g3_socket::tcp::set_raw_opts(
            self.ctx.tcp_client_socket,
            &self.ctx.server_config.tcp_misc_opts,
            true,
        )
        .map_err(|_| ServerTaskError::InternalServerError("failed to set client socket options"))?;

        let next_addr = self.service.select_addr(self.ctx.client_addr.ip());

        self.task_notes.stage = ServerTaskStage::Connecting;

        let socket = g3_socket::tcp::new_socket_to(
            next_addr.ip(),
            None,
            &Default::default(),
            &Default::default(),
            true,
        )
        .map_err(|e| ServerTaskError::UnclassifiedError(anyhow!("setup socket failed: {e:?}")))?;
        let stream = socket.connect(next_addr).await.map_err(|e| {
            ServerTaskError::UnclassifiedError(anyhow!("failed to connect to {next_addr}: {e:?}"))
        })?;
        let (ups_r, ups_w) = stream.into_split();

        self.task_notes.stage = ServerTaskStage::Connected;

        self.run_connected(ssl_stream, ups_r, ups_w).await
    }

    async fn run_connected<S, UR, UW>(
        &mut self,
        ssl_stream: SslStream<LimitedStream<S>>,
        ups_r: UR,
        ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        S: AsyncRead + AsyncWrite,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        self.task_notes.mark_relaying();
        self.relay(ssl_stream, ups_r, ups_w).await
    }

    async fn relay<S, UR, UW>(
        &mut self,
        mut ssl_stream: SslStream<LimitedStream<S>>,
        mut ups_r: UR,
        mut ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        S: AsyncRead + AsyncWrite,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        self.reset_clt_limit_and_stats(&mut ssl_stream);
        let (mut clt_r, mut clt_w) = tokio::io::split(ssl_stream);

        let copy_config = LimitedCopyConfig::default();
        let mut clt_to_ups = LimitedCopy::new(&mut clt_r, &mut ups_w, &copy_config);
        let mut ups_to_clt = LimitedCopy::new(&mut ups_r, &mut clt_w, &copy_config);

        let idle_duration = self.ctx.server_config.task_idle_check_duration;
        let task_idle_max_count = self
            .host
            .config
            .task_idle_max_count
            .unwrap_or(self.ctx.server_config.task_idle_max_count);
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;
        loop {
            tokio::select! {
                biased;

                r = &mut clt_to_ups => {
                    let _ = ups_to_clt.write_flush().await;
                    return match r {
                        Ok(_) => Err(ServerTaskError::ClosedByClient),
                        Err(LimitedCopyError::ReadFailed(e)) => Err(ServerTaskError::ClientTcpReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(ServerTaskError::UpstreamWriteFailed(e)),
                    };
                }
                r = &mut ups_to_clt => {
                    let _ = clt_to_ups.write_flush().await;
                    return match r {
                        Ok(_) => Err(ServerTaskError::ClosedByUpstream),
                        Err(LimitedCopyError::ReadFailed(e)) => Err(ServerTaskError::UpstreamReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(ServerTaskError::ClientTcpWriteFailed(e)),
                    };
                }
                _ = idle_interval.tick() => {
                    if clt_to_ups.is_idle() && ups_to_clt.is_idle() {
                        idle_count += 1;

                        if idle_count >= task_idle_max_count {
                            return Err(ServerTaskError::Idle(idle_duration, idle_count));
                        }
                    } else {
                        idle_count = 0;

                        clt_to_ups.reset_active();
                        ups_to_clt.reset_active();
                    }

                    if self.ctx.server_quit_policy.force_quit() {
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            };
        }
    }

    fn reset_clt_limit_and_stats<S>(&self, ssl_stream: &mut SslStream<LimitedStream<S>>)
    where
        S: AsyncRead + AsyncWrite,
    {
        // reset io limit
        if let Some(limit) = &self.host.config.tcp_sock_speed_limit {
            let limit = self
                .ctx
                .server_config
                .tcp_sock_speed_limit
                .shrink_as_smaller(limit);
            ssl_stream
                .get_mut()
                .reset_limit(limit.shift_millis, limit.max_north, limit.max_south);
        }

        // reset io stats
        // TODO add host level stats
        let clt_wrapper_stats =
            OpensslRelayTaskCltWrapperStats::new(&self.ctx.server_stats, &self.task_stats);
        ssl_stream
            .get_mut()
            .reset_stats(Arc::new(clt_wrapper_stats));
    }
}
