/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use bytes::BytesMut;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_daemon::server::ServerQuitPolicy;
use g3_daemon::stat::task::{TcpStreamConnectionStats, TcpStreamTaskStats};
use g3_dpi::Protocol;
use g3_io_ext::{
    FlexBufReader, IdleInterval, LimitedReader, LimitedWriter, StreamCopy, StreamCopyConfig,
};
use g3_types::net::UpstreamAddr;

use super::CommonTaskContext;
use crate::audit::AuditContext;
use crate::auth::User;
use crate::inspect::{StreamInspectContext, StreamInspection, StreamTransitTask};
use crate::log::task::tcp_connect::TaskLogForTcpConnect;
use crate::module::tcp_connect::{TcpConnectTaskConf, TcpConnectTaskNotes};
use crate::serve::tcp_stream::{TcpStreamServerAliveTaskGuard, TcpStreamTaskCltWrapperStats};
use crate::serve::{ServerTaskError, ServerTaskNotes, ServerTaskResult, ServerTaskStage};

pub(crate) struct TcpStreamTask {
    ctx: CommonTaskContext,
    upstream: UpstreamAddr,
    protocol: Protocol,
    tcp_notes: TcpConnectTaskNotes,
    task_notes: ServerTaskNotes,
    task_stats: Arc<TcpStreamTaskStats>,
    audit_ctx: AuditContext,
    _alive_guard: Option<TcpStreamServerAliveTaskGuard>,
}

impl TcpStreamTask {
    pub(crate) fn new(
        ctx: CommonTaskContext,
        audit_ctx: AuditContext,
        protocol: Protocol,
        upstream: UpstreamAddr,
        wait_time: Duration,
        pre_handshake_stats: TcpStreamConnectionStats,
    ) -> Self {
        let task_notes = ServerTaskNotes::new(ctx.cc_info.clone(), None, wait_time);
        TcpStreamTask {
            ctx,
            upstream,
            protocol,
            tcp_notes: TcpConnectTaskNotes::default(),
            task_notes,
            task_stats: Arc::new(TcpStreamTaskStats::with_clt_stats(pre_handshake_stats)),
            audit_ctx,
            _alive_guard: None,
        }
    }

    fn get_log_context(&self) -> Option<TaskLogForTcpConnect<'_>> {
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

    pub(crate) async fn into_running<R, W>(
        mut self,
        clt_r: LimitedReader<R>,
        clt_r_buf: BytesMut,
        clt_w: LimitedWriter<W>,
    ) where
        R: AsyncRead + Send + Sync + Unpin + 'static,
        W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        self.pre_start();
        let e = match self.run(clt_r, clt_r_buf, clt_w).await {
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

    async fn run<R, W>(
        &mut self,
        clt_r: LimitedReader<R>,
        clt_r_buf: BytesMut,
        clt_w: LimitedWriter<W>,
    ) -> ServerTaskResult<()>
    where
        R: AsyncRead + Send + Sync + Unpin + 'static,
        W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        self.task_stats.clt.read.add_bytes(clt_r_buf.len() as u64);

        // set client side socket options
        self.ctx
            .cc_info
            .tcp_sock_set_raw_opts(&self.ctx.server_config.tcp_misc_opts, true)
            .map_err(|_| {
                ServerTaskError::InternalServerError("failed to set client socket options")
            })?;

        self.task_notes.stage = ServerTaskStage::Connecting;

        let task_conf = TcpConnectTaskConf {
            upstream: &self.upstream,
        };
        let (ups_r, ups_w) = self
            .ctx
            .escaper
            .tcp_setup_connection(
                &task_conf,
                &mut self.tcp_notes,
                &self.task_notes,
                self.task_stats.clone(),
                &mut self.audit_ctx,
            )
            .await?;

        self.task_notes.stage = ServerTaskStage::Connected;
        self.run_connected(clt_r, clt_r_buf, clt_w, ups_r, ups_w)
            .await
    }

    async fn run_connected<CR, CW, UR, UW>(
        &mut self,
        clt_r: LimitedReader<CR>,
        clt_r_buf: BytesMut,
        clt_w: LimitedWriter<CW>,
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
        self.relay(clt_r, clt_r_buf, clt_w, ups_r, ups_w).await
    }

    async fn relay<CR, CW, UR, UW>(
        &mut self,
        mut clt_r: LimitedReader<CR>,
        clt_r_buf: BytesMut,
        mut clt_w: LimitedWriter<CW>,
        mut ups_r: UR,
        mut ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Send + Sync + Unpin + 'static,
        CW: AsyncWrite + Send + Sync + Unpin + 'static,
        UR: AsyncRead + Send + Sync + Unpin + 'static,
        UW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let (clt_r_stats, clt_w_stats) =
            TcpStreamTaskCltWrapperStats::new_pair(&self.ctx.server_stats, &self.task_stats);
        clt_r.reset_stats(clt_r_stats);
        clt_w.reset_stats(clt_w_stats);

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
            let protocol_inspector = ctx.protocol_inspector(None);
            match self.protocol {
                Protocol::TlsModern => {
                    if let Some(tls_interception) = ctx.tls_interception() {
                        let mut tls_obj = crate::inspect::tls::TlsInterceptObject::new(
                            ctx,
                            self.upstream.clone(),
                            tls_interception,
                        );
                        tls_obj.set_io(
                            clt_r_buf,
                            Box::new(clt_r),
                            Box::new(clt_w),
                            Box::new(ups_r),
                            Box::new(ups_w),
                        );
                        return StreamInspection::TlsModern(tls_obj)
                            .into_loop_inspection(protocol_inspector)
                            .await;
                    }
                }
                #[cfg(feature = "vendored-tongsuo")]
                Protocol::TlsTlcp => {
                    if let Some(tls_interception) = ctx.tls_interception() {
                        let mut tls_obj = crate::inspect::tls::TlsInterceptObject::new(
                            ctx,
                            self.upstream.clone(),
                            tls_interception,
                        );
                        tls_obj.set_io(
                            clt_r_buf,
                            Box::new(clt_r),
                            Box::new(clt_w),
                            Box::new(ups_r),
                            Box::new(ups_w),
                        );
                        return StreamInspection::TlsTlcp(tls_obj)
                            .into_loop_inspection(protocol_inspector)
                            .await;
                    }
                }
                Protocol::Http1 => {
                    let mut h1_obj = crate::inspect::http::H1InterceptObject::new(ctx);
                    h1_obj.set_io(
                        FlexBufReader::with_bytes(clt_r_buf, Box::new(clt_r)),
                        Box::new(clt_w),
                        Box::new(ups_r),
                        Box::new(ups_w),
                    );
                    return StreamInspection::H1(h1_obj)
                        .into_loop_inspection(protocol_inspector)
                        .await;
                }
                _ => {
                    return Err(ServerTaskError::InvalidClientProtocol(
                        "unsupported client protocol",
                    ));
                }
            }
        }

        let copy_config = self.ctx.server_config.tcp_copy;
        let clt_to_ups =
            StreamCopy::with_data(&mut clt_r, &mut ups_w, &copy_config, clt_r_buf.into());
        let ups_to_clt = StreamCopy::new(&mut ups_r, &mut clt_w, &copy_config);
        self.transit_transparent2(clt_to_ups, ups_to_clt).await
    }
}

impl StreamTransitTask for TcpStreamTask {
    fn copy_config(&self) -> StreamCopyConfig {
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
