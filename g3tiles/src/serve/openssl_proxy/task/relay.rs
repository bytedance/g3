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
use tokio::io::{AsyncRead, AsyncWrite};

use g3_daemon::server::ServerQuitPolicy;
use g3_daemon::stat::task::{TcpStreamConnectionStats, TcpStreamTaskStats};
use g3_io_ext::{AsyncStream, IdleInterval, LimitedCopyConfig, LimitedStream, OnceBufReader};
use g3_openssl::SslStream;
use g3_types::limit::GaugeSemaphorePermit;

use super::CommonTaskContext;
use crate::backend::ArcBackend;
use crate::config::server::ServerConfig;
use crate::log::task::tcp_connect::TaskLogForTcpConnect;
use crate::module::stream::{StreamRelayTaskCltWrapperStats, StreamTransitTask};
use crate::serve::openssl_proxy::OpensslHost;
use crate::serve::{ServerTaskError, ServerTaskNotes, ServerTaskResult, ServerTaskStage};

pub(crate) struct OpensslRelayTask {
    ctx: CommonTaskContext,
    host: Arc<OpensslHost>,
    backend: ArcBackend,
    task_notes: ServerTaskNotes,
    task_stats: Arc<TcpStreamTaskStats>,
    alive_permit: Option<GaugeSemaphorePermit>,
}

impl OpensslRelayTask {
    pub(crate) fn new(
        ctx: CommonTaskContext,
        host: Arc<OpensslHost>,
        backend: ArcBackend,
        wait_time: Duration,
        pre_handshake_stats: Arc<TcpStreamConnectionStats>,
        alive_permit: Option<GaugeSemaphorePermit>,
    ) -> Self {
        let task_notes = ServerTaskNotes::new(ctx.cc_info.clone(), wait_time);
        OpensslRelayTask {
            ctx,
            host,
            backend,
            task_notes,
            task_stats: Arc::new(TcpStreamTaskStats::with_clt_stats(
                pre_handshake_stats.as_ref().clone(),
            )),
            alive_permit,
        }
    }

    fn get_log_context(&self) -> TaskLogForTcpConnect {
        TaskLogForTcpConnect {
            task_notes: &self.task_notes,
            client_rd_bytes: self.task_stats.clt.read.get_bytes(),
            client_wr_bytes: self.task_stats.clt.write.get_bytes(),
            remote_rd_bytes: self.task_stats.ups.read.get_bytes(),
            remote_wr_bytes: self.task_stats.ups.write.get_bytes(),
        }
    }

    pub(crate) async fn into_running<S>(
        mut self,
        ssl_stream: SslStream<OnceBufReader<LimitedStream<S>>>,
    ) where
        S: AsyncRead + AsyncWrite + Unpin,
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
            self.ctx.client_addr(),
            self.ctx.server_config.server_type(),
            self.ctx.server_config.name(),
        );
        self.ctx.server_stats.add_task();
        self.ctx.server_stats.inc_alive_task();
    }

    fn pre_stop(&mut self) {
        if let Some(permit) = self.alive_permit.take() {
            drop(permit);
        }
        self.ctx.server_stats.dec_alive_task();
    }

    async fn run<S>(
        &mut self,
        ssl_stream: SslStream<OnceBufReader<LimitedStream<S>>>,
    ) -> ServerTaskResult<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        self.task_notes.stage = ServerTaskStage::Preparing;

        // set client side socket options
        self.ctx
            .cc_info
            .tcp_sock_set_raw_opts(&self.ctx.server_config.tcp_misc_opts, true)
            .map_err(|_| {
                ServerTaskError::InternalServerError("failed to set client socket options")
            })?;

        self.task_notes.stage = ServerTaskStage::Connecting;

        let (ups_r, ups_w) = self.backend.stream_connect(&self.task_notes).await?;

        self.task_notes.stage = ServerTaskStage::Connected;

        self.run_connected(ssl_stream, ups_r, ups_w).await
    }

    async fn run_connected<S, UR, UW>(
        &mut self,
        ssl_stream: SslStream<OnceBufReader<LimitedStream<S>>>,
        ups_r: UR,
        ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        self.task_notes.mark_relaying();
        self.relay(ssl_stream, ups_r, ups_w).await
    }

    async fn relay<S, UR, UW>(
        &mut self,
        mut ssl_stream: SslStream<OnceBufReader<LimitedStream<S>>>,
        ups_r: UR,
        ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        self.reset_clt_limit_and_stats(&mut ssl_stream);
        let (clt_r, clt_w) = ssl_stream.into_split();

        self.transit_transparent(clt_r, clt_w, ups_r, ups_w).await
    }

    fn reset_clt_limit_and_stats<S>(
        &self,
        ssl_stream: &mut SslStream<OnceBufReader<LimitedStream<S>>>,
    ) where
        S: AsyncRead + AsyncWrite,
    {
        // reset io limit
        if let Some(limit) = &self.host.config.tcp_sock_speed_limit {
            let limit = self
                .ctx
                .server_config
                .tcp_sock_speed_limit
                .shrink_as_smaller(limit);
            ssl_stream.get_mut().inner_mut().reset_local_limit(
                limit.shift_millis,
                limit.max_north,
                limit.max_south,
            );
        }

        // reset io stats
        // TODO add host level stats
        let clt_wrapper_stats =
            StreamRelayTaskCltWrapperStats::new(&self.ctx.server_stats, &self.task_stats);
        ssl_stream
            .get_mut()
            .inner_mut()
            .reset_stats(Arc::new(clt_wrapper_stats));
    }
}

impl StreamTransitTask for OpensslRelayTask {
    fn copy_config(&self) -> LimitedCopyConfig {
        self.ctx.server_config.tcp_copy
    }

    fn idle_check_interval(&self) -> IdleInterval {
        self.ctx.idle_wheel.register()
    }

    fn max_idle_count(&self) -> usize {
        self.host
            .config
            .task_idle_max_count
            .unwrap_or(self.ctx.server_config.task_idle_max_count)
    }

    fn log_client_shutdown(&self) {
        self.get_log_context()
            .log_client_shutdown(&self.ctx.task_logger);
    }

    fn log_upstream_shutdown(&self) {
        self.get_log_context()
            .log_upstream_shutdown(&self.ctx.task_logger);
    }

    fn log_periodic(&self) {
        self.get_log_context().log_periodic(&self.ctx.task_logger);
    }

    fn log_flush_interval(&self) -> Option<Duration> {
        None
    }

    fn quit_policy(&self) -> &ServerQuitPolicy {
        self.ctx.server_quit_policy.as_ref()
    }
}
