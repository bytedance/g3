/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::server::TlsStream;

use g3_daemon::server::ServerQuitPolicy;
use g3_daemon::stat::task::{TcpStreamConnectionStats, TcpStreamTaskStats};
use g3_io_ext::{AsyncStream, IdleInterval, LimitedStream, StreamCopyConfig};
use g3_types::limit::GaugeSemaphorePermit;

use super::CommonTaskContext;
use crate::backend::ArcBackend;
use crate::log::task::tcp_connect::TaskLogForTcpConnect;
use crate::module::stream::{
    StreamRelayTaskCltWrapperStats, StreamServerAliveTaskGuard, StreamTransitTask,
};
use crate::serve::rustls_proxy::RustlsHost;
use crate::serve::{ServerTaskError, ServerTaskNotes, ServerTaskResult, ServerTaskStage};

pub(crate) struct RustlsRelayTask {
    ctx: CommonTaskContext,
    host: Arc<RustlsHost>,
    backend: ArcBackend,
    task_notes: ServerTaskNotes,
    task_stats: Arc<TcpStreamTaskStats>,
    _alive_permit: Option<GaugeSemaphorePermit>,
    _alive_guard: Option<StreamServerAliveTaskGuard>,
}

impl RustlsRelayTask {
    pub(crate) fn new(
        ctx: CommonTaskContext,
        host: Arc<RustlsHost>,
        backend: ArcBackend,
        wait_time: Duration,
        pre_handshake_stats: Arc<TcpStreamConnectionStats>,
        alive_permit: Option<GaugeSemaphorePermit>,
    ) -> Self {
        let task_notes = ServerTaskNotes::new(ctx.cc_info.clone(), wait_time);
        RustlsRelayTask {
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

    pub(crate) async fn into_running<S>(mut self, tls_stream: TlsStream<LimitedStream<S>>)
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        self.pre_start();
        if let Err(e) = self.run(tls_stream).await
            && let Some(log_ctx) = self.get_log_context()
        {
            log_ctx.log(e);
        }
    }

    fn pre_start(&mut self) {
        self._alive_guard = Some(self.ctx.server_stats.add_task());

        if self.ctx.server_config.flush_task_log_on_created
            && let Some(log_ctx) = self.get_log_context()
        {
            log_ctx.log_created();
        }
    }

    async fn run<S>(&mut self, tls_stream: TlsStream<LimitedStream<S>>) -> ServerTaskResult<()>
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

        self.run_connected(tls_stream, ups_r, ups_w).await
    }

    async fn run_connected<S, UR, UW>(
        &mut self,
        tls_stream: TlsStream<LimitedStream<S>>,
        ups_r: UR,
        ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        if self.ctx.server_config.flush_task_log_on_connected
            && let Some(log_ctx) = self.get_log_context()
        {
            log_ctx.log_connected();
        }

        self.task_notes.mark_relaying();
        self.relay(tls_stream, ups_r, ups_w).await
    }

    async fn relay<S, UR, UW>(
        &mut self,
        mut tls_stream: TlsStream<LimitedStream<S>>,
        ups_r: UR,
        ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        self.reset_clt_limit_and_stats(&mut tls_stream);
        let (clt_r, clt_w) = tls_stream.into_split();

        self.transit_transparent(clt_r, clt_w, ups_r, ups_w).await
    }

    fn reset_clt_limit_and_stats<S>(&self, tls_stream: &mut TlsStream<LimitedStream<S>>)
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
            tls_stream.get_mut().0.reset_local_limit(
                limit.shift_millis,
                limit.max_north,
                limit.max_south,
            );
        }

        // reset io stats
        // TODO add host level stats
        let clt_wrapper_stats =
            StreamRelayTaskCltWrapperStats::new(&self.ctx.server_stats, &self.task_stats);
        tls_stream
            .get_mut()
            .0
            .reset_stats(Arc::new(clt_wrapper_stats));
    }
}

impl StreamTransitTask for RustlsRelayTask {
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
