/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};

use g3_daemon::server::ServerQuitPolicy;
use g3_daemon::stat::task::{TcpStreamConnectionStats, TcpStreamTaskStats};
use g3_io_ext::{AsyncStream, IdleInterval, LimitedStream, OnceBufReader, StreamCopyConfig};
use g3_openssl::SslStream;
use g3_types::limit::GaugeSemaphorePermit;

use super::CommonTaskContext;
use crate::backend::ArcBackend;
use crate::log::task::tcp_connect::TaskLogForTcpConnect;
use crate::module::stream::{
    StreamRelayTaskCltWrapperStats, StreamServerAliveTaskGuard, StreamTransitTask,
};
use crate::serve::openssl_proxy::OpensslHost;
use crate::serve::{ServerTaskError, ServerTaskNotes, ServerTaskResult, ServerTaskStage};

pub(crate) struct OpensslRelayTask {
    ctx: CommonTaskContext,
    host: Arc<OpensslHost>,
    backend: ArcBackend,
    task_notes: ServerTaskNotes,
    task_stats: Arc<TcpStreamTaskStats>,
    _alive_permit: Option<GaugeSemaphorePermit>,
    _alive_guard: Option<StreamServerAliveTaskGuard>,
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
            _alive_permit: alive_permit,
            _alive_guard: None,
        }
    }

    fn get_log_context(&self) -> Option<TaskLogForTcpConnect<'_>> {
        self.ctx
            .task_logger
            .as_ref()
            .map(|logger| TaskLogForTcpConnect {
                logger,
                task_notes: &self.task_notes,
                client_rd_bytes: self.task_stats.clt.read.get_bytes(),
                client_wr_bytes: self.task_stats.clt.write.get_bytes(),
                remote_rd_bytes: self.task_stats.ups.read.get_bytes(),
                remote_wr_bytes: self.task_stats.ups.write.get_bytes(),
            })
    }

    pub(crate) async fn into_running<S>(
        mut self,
        ssl_stream: SslStream<OnceBufReader<LimitedStream<S>>>,
    ) where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        self.pre_start();
        if let Err(e) = self.run(ssl_stream).await {
            if let Some(log_ctx) = self.get_log_context() {
                log_ctx.log(e);
            }
        }
    }

    fn pre_start(&mut self) {
        self._alive_guard = Some(self.ctx.server_stats.add_task());

        if self.ctx.server_config.flush_task_log_on_created {
            if let Some(log_ctx) = self.get_log_context() {
                log_ctx.log_created();
            }
        }
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
        if self.ctx.server_config.flush_task_log_on_connected {
            if let Some(log_ctx) = self.get_log_context() {
                log_ctx.log_connected();
            }
        }

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
    fn copy_config(&self) -> StreamCopyConfig {
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
        if let Some(log_ctx) = self.get_log_context() {
            log_ctx.log_client_shutdown();
        }
    }

    fn log_upstream_shutdown(&self) {
        if let Some(log_ctx) = self.get_log_context() {
            log_ctx.log_upstream_shutdown();
        }
    }

    fn log_periodic(&self) {
        if let Some(log_ctx) = self.get_log_context() {
            log_ctx.log_periodic();
        }
    }

    fn log_flush_interval(&self) -> Option<Duration> {
        self.ctx.log_flush_interval()
    }

    fn quit_policy(&self) -> &ServerQuitPolicy {
        self.ctx.server_quit_policy.as_ref()
    }
}
