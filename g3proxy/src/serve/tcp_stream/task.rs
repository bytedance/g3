/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};

use g3_daemon::server::ServerQuitPolicy;
use g3_daemon::stat::task::TcpStreamTaskStats;
use g3_io_ext::{IdleInterval, LimitedCopyConfig, LimitedReader, LimitedWriter};
use g3_types::net::UpstreamAddr;

use super::common::CommonTaskContext;
use super::stats::{TcpStreamServerAliveTaskGuard, TcpStreamTaskCltWrapperStats};
use crate::audit::AuditContext;
use crate::auth::User;
use crate::inspect::{StreamInspectContext, StreamTransitTask};
use crate::log::task::tcp_connect::TaskLogForTcpConnect;
use crate::module::tcp_connect::{TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf};
use crate::serve::{ServerTaskError, ServerTaskNotes, ServerTaskResult, ServerTaskStage};

pub(super) struct TcpStreamTask {
    ctx: CommonTaskContext,
    upstream: UpstreamAddr,
    tcp_notes: TcpConnectTaskNotes,
    task_notes: ServerTaskNotes,
    task_stats: Arc<TcpStreamTaskStats>,
    audit_ctx: AuditContext,
    _alive_guard: Option<TcpStreamServerAliveTaskGuard>,
}

impl TcpStreamTask {
    pub(super) fn new(
        ctx: CommonTaskContext,
        upstream: &UpstreamAddr,
        audit_ctx: AuditContext,
    ) -> Self {
        let task_notes = ServerTaskNotes::new(ctx.cc_info.clone(), None, Duration::ZERO);
        TcpStreamTask {
            ctx,
            upstream: upstream.clone(),
            tcp_notes: TcpConnectTaskNotes::default(),
            task_notes,
            task_stats: Arc::new(TcpStreamTaskStats::default()),
            audit_ctx,
            _alive_guard: None,
        }
    }

    fn get_log_context(&self) -> Option<TaskLogForTcpConnect> {
        self.ctx
            .task_logger
            .as_ref()
            .map(|logger| TaskLogForTcpConnect {
                logger,
                upstream: &self.upstream,
                task_notes: &self.task_notes,
                tcp_notes: &self.tcp_notes,
                client_rd_bytes: self.task_stats.clt.read.get_bytes(),
                client_wr_bytes: self.task_stats.clt.write.get_bytes(),
                remote_rd_bytes: self.task_stats.ups.read.get_bytes(),
                remote_wr_bytes: self.task_stats.ups.write.get_bytes(),
            })
    }

    pub(super) async fn into_running<CR, CW>(mut self, clt_r: CR, clt_w: CW)
    where
        CR: AsyncRead + Send + Sync + Unpin + 'static,
        CW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        self.pre_start();
        let (clt_r, clt_w) = self.setup_limit_and_stats(clt_r, clt_w);
        let e = match self.run(clt_r, clt_w).await {
            Ok(_) => ServerTaskError::Finished,
            Err(e) => e,
        };
        if let Some(log_ctx) = self.get_log_context() {
            log_ctx.log(e);
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

    async fn run<CR, CW>(&mut self, clt_r: CR, clt_w: CW) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Send + Sync + Unpin + 'static,
        CW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        // set client side socket options
        self.ctx
            .cc_info
            .tcp_sock_set_raw_opts(&self.ctx.server_config.tcp_misc_opts, true)
            .map_err(|_| {
                ServerTaskError::InternalServerError("failed to set client socket options")
            })?;

        self.task_notes.stage = ServerTaskStage::Connecting;
        let (ups_r, ups_w) = if let Some(tls_client_config) = &self.ctx.tls_client_config {
            let tls_name = self
                .ctx
                .server_config
                .upstream_tls_name
                .as_ref()
                .unwrap_or_else(|| self.upstream.host());
            let task_conf = TlsConnectTaskConf {
                tcp: TcpConnectTaskConf {
                    upstream: &self.upstream,
                },
                tls_config: tls_client_config,
                tls_name,
            };
            self.ctx
                .escaper
                .tls_setup_connection(
                    &task_conf,
                    &mut self.tcp_notes,
                    &self.task_notes,
                    self.task_stats.clone(),
                    &mut self.audit_ctx,
                )
                .await?
        } else {
            let task_conf = TcpConnectTaskConf {
                upstream: &self.upstream,
            };
            self.ctx
                .escaper
                .tcp_setup_connection(
                    &task_conf,
                    &mut self.tcp_notes,
                    &self.task_notes,
                    self.task_stats.clone(),
                    &mut self.audit_ctx,
                )
                .await?
        };

        self.task_notes.stage = ServerTaskStage::Connected;
        self.run_connected(clt_r, clt_w, ups_r, ups_w).await
    }

    async fn run_connected<CR, CW, UR, UW>(
        &mut self,
        clt_r: CR,
        clt_w: CW,
        ups_r: UR,
        ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Send + Sync + Unpin + 'static,
        CW: AsyncWrite + Send + Sync + Unpin + 'static,
        UR: AsyncRead + Send + Sync + Unpin + 'static,
        UW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        if self.ctx.server_config.flush_task_log_on_connected {
            if let Some(log_ctx) = self.get_log_context() {
                log_ctx.log_connected();
            }
        }
        self.task_notes.mark_relaying();
        self.relay(clt_r, clt_w, ups_r, ups_w).await
    }

    async fn relay<CR, CW, UR, UW>(
        &mut self,
        clt_r: CR,
        clt_w: CW,
        ups_r: UR,
        ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Send + Sync + Unpin + 'static,
        CW: AsyncWrite + Send + Sync + Unpin + 'static,
        UR: AsyncRead + Send + Sync + Unpin + 'static,
        UW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        if let Some(audit_handle) = self.audit_ctx.check_take_handle() {
            let ctx = StreamInspectContext::new(
                audit_handle,
                self.ctx.server_config.clone(),
                self.ctx.server_stats.clone(),
                self.ctx.server_quit_policy.clone(),
                self.ctx.idle_wheel.clone(),
                &self.task_notes,
                &self.tcp_notes,
            );
            crate::inspect::stream::transit_with_inspection(
                clt_r,
                clt_w,
                ups_r,
                ups_w,
                ctx,
                self.upstream.clone(),
                None,
            )
            .await
        } else {
            self.transit_transparent(clt_r, clt_w, ups_r, ups_w).await
        }
    }

    fn setup_limit_and_stats<CR, CW>(
        &self,
        clt_r: CR,
        clt_w: CW,
    ) -> (LimitedReader<CR>, LimitedWriter<CW>)
    where
        CR: AsyncRead,
        CW: AsyncWrite,
    {
        let (clt_r_stats, clt_w_stats) =
            TcpStreamTaskCltWrapperStats::new_pair(&self.ctx.server_stats, &self.task_stats);
        let clt_speed_limit = &self.ctx.server_config.tcp_sock_speed_limit;

        let clt_r = LimitedReader::local_limited(
            clt_r,
            clt_speed_limit.shift_millis,
            clt_speed_limit.max_north,
            clt_r_stats,
        );
        let clt_w = LimitedWriter::local_limited(
            clt_w,
            clt_speed_limit.shift_millis,
            clt_speed_limit.max_south,
            clt_w_stats,
        );

        (clt_r, clt_w)
    }
}

impl StreamTransitTask for TcpStreamTask {
    fn copy_config(&self) -> LimitedCopyConfig {
        self.ctx.server_config.tcp_copy
    }

    fn idle_check_interval(&self) -> IdleInterval {
        self.ctx.idle_wheel.register()
    }

    fn max_idle_count(&self) -> usize {
        self.ctx.server_config.task_idle_max_count
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

    fn user(&self) -> Option<&User> {
        None
    }
}
