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
use tokio_rustls::server::TlsStream;

use g3_daemon::server::ServerQuitPolicy;
use g3_daemon::stat::task::{TcpStreamConnectionStats, TcpStreamTaskStats};
use g3_io_ext::{AsyncStream, IdleInterval, LimitedCopyConfig, LimitedStream};
use g3_types::limit::GaugeSemaphorePermit;

use super::CommonTaskContext;
use crate::backend::ArcBackend;
use crate::config::server::ServerConfig;
use crate::log::task::tcp_connect::TaskLogForTcpConnect;
use crate::module::stream::{StreamRelayTaskCltWrapperStats, StreamTransitTask};
use crate::serve::rustls_proxy::RustlsHost;
use crate::serve::{ServerTaskError, ServerTaskNotes, ServerTaskResult, ServerTaskStage};

pub(crate) struct RustlsRelayTask {
    ctx: CommonTaskContext,
    host: Arc<RustlsHost>,
    backend: ArcBackend,
    task_notes: ServerTaskNotes,
    task_stats: Arc<TcpStreamTaskStats>,
    alive_permit: Option<GaugeSemaphorePermit>,
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

    pub(crate) async fn into_running<S>(mut self, tls_stream: TlsStream<LimitedStream<S>>)
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        self.pre_start();
        if let Err(e) = self.run(tls_stream).await {
            self.get_log_context().log(&self.ctx.task_logger, &e)
        }
        self.pre_stop();
    }

    fn pre_start(&self) {
        debug!(
            "RustlsProxy: new client from {} to {} server {}",
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
