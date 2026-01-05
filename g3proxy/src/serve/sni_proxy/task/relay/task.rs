/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::borrow::Cow;
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
use g3_types::acl::AclAction;
use g3_types::net::UpstreamAddr;

use super::CommonTaskContext;
use crate::audit::AuditContext;
use crate::auth::User;
use crate::config::server::ServerConfig;
use crate::inspect::{StreamInspectContext, StreamInspection, StreamTransitTask};
use crate::log::task::tcp_connect::TaskLogForTcpConnect;
use crate::module::tcp_connect::{TcpConnectTaskConf, TcpConnectTaskNotes};
use crate::serve::tcp_stream::{TcpStreamServerAliveTaskGuard, TcpStreamTaskCltWrapperStats};
use crate::serve::{
    ServerStats, ServerTaskError, ServerTaskForbiddenError, ServerTaskNotes, ServerTaskResult,
    ServerTaskStage,
};

pub(crate) struct TcpStreamTask {
    ctx: CommonTaskContext,
    upstream: UpstreamAddr,
    protocol: Protocol,
    tcp_notes: TcpConnectTaskNotes,
    task_notes: ServerTaskNotes,
    task_stats: Arc<TcpStreamTaskStats>,
    audit_ctx: AuditContext,
    started: bool,
    _alive_guard: Option<TcpStreamServerAliveTaskGuard>,
}

impl Drop for TcpStreamTask {
    fn drop(&mut self) {
        if self.started {
            self.post_stop();
            self.started = false;
        }
    }
}

impl TcpStreamTask {
    pub(crate) fn new(
        ctx: CommonTaskContext,
        audit_ctx: AuditContext,
        protocol: Protocol,
        upstream: UpstreamAddr,
        pre_handshake_stats: TcpStreamConnectionStats,
        task_notes: ServerTaskNotes,
    ) -> Self {
        TcpStreamTask {
            ctx,
            upstream,
            protocol,
            tcp_notes: TcpConnectTaskNotes::default(),
            task_notes,
            task_stats: Arc::new(TcpStreamTaskStats::with_clt_stats(pre_handshake_stats)),
            audit_ctx,
            started: false,
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

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.foreach_req_stats(|s| {
                s.req_total.add_tcp_connect();
                s.req_alive.add_tcp_connect();
            });
        }

        if self.ctx.server_config.flush_task_log_on_created
            && let Some(log_ctx) = self.get_log_context()
        {
            log_ctx.log_created();
        }

        self.started = true;
    }

    fn post_stop(&mut self) {
        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.foreach_req_stats(|s| {
                s.req_alive.del_tcp_connect();
            });

            if let Some(user_req_alive_permit) = self.task_notes.user_req_alive_permit.take() {
                drop(user_req_alive_permit);
            }
        }
    }

    async fn handle_user_upstream_acl_action(&mut self, action: AclAction) -> ServerTaskResult<()> {
        let forbid = match action {
            AclAction::Permit => false,
            AclAction::PermitAndLog => {
                // TODO log permit
                false
            }
            AclAction::Forbid => true,
            AclAction::ForbidAndLog => {
                // TODO log forbid
                true
            }
        };
        if forbid {
            Err(ServerTaskError::ForbiddenByRule(
                ServerTaskForbiddenError::DestDenied,
            ))
        } else {
            Ok(())
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

        let tcp_client_misc_opts;

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            let user_ctx = user_ctx.clone();

            if user_ctx.check_rate_limit().is_err() {
                return Err(ServerTaskError::ForbiddenByRule(
                    ServerTaskForbiddenError::RateLimited,
                ));
            }

            match user_ctx.acquire_request_semaphore() {
                Ok(permit) => self.task_notes.user_req_alive_permit = Some(permit),
                Err(_) => {
                    return Err(ServerTaskError::ForbiddenByRule(
                        ServerTaskForbiddenError::FullyLoaded,
                    ));
                }
            }

            let action = user_ctx.check_upstream(&self.upstream);
            self.handle_user_upstream_acl_action(action).await?;

            let user_config = user_ctx.user_config();

            tcp_client_misc_opts =
                user_config.tcp_client_misc_opts(&self.ctx.server_config.tcp_misc_opts);
            //
        } else {
            tcp_client_misc_opts = Cow::Borrowed(&self.ctx.server_config.tcp_misc_opts);
        }

        // set client side socket options
        self.ctx
            .cc_info
            .tcp_sock_set_raw_opts(&tcp_client_misc_opts, true)
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
        if self.ctx.server_config.flush_task_log_on_connected
            && let Some(log_ctx) = self.get_log_context()
        {
            log_ctx.log_connected();
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
        self.reset_clt(&mut clt_r, &mut clt_w);

        if let Some(audit_handle) = self.audit_ctx.check_take_handle() {
            let audit_task = self
                .task_notes
                .user_ctx()
                .map(|ctx| {
                    let user_config = &ctx.user_config().audit;
                    user_config.enable_protocol_inspection
                        && user_config
                            .do_task_audit()
                            .unwrap_or_else(|| audit_handle.do_task_audit())
                })
                .unwrap_or_else(|| audit_handle.do_task_audit());

            if audit_task {
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
        }

        let copy_config = self.ctx.server_config.tcp_copy;
        let clt_to_ups =
            StreamCopy::with_data(&mut clt_r, &mut ups_w, &copy_config, clt_r_buf.into());
        let ups_to_clt = StreamCopy::new(&mut ups_r, &mut clt_w, &copy_config);
        self.transit_transparent2(clt_to_ups, ups_to_clt).await
    }

    fn reset_clt<R, W>(&self, clt_r: &mut LimitedReader<R>, clt_w: &mut LimitedWriter<W>) {
        let mut wrapper_stats =
            TcpStreamTaskCltWrapperStats::new(&self.ctx.server_stats, &self.task_stats);

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            wrapper_stats.push_user_io_stats(user_ctx.fetch_traffic_stats(
                self.ctx.server_config.name(),
                self.ctx.server_stats.share_extra_tags(),
            ));

            let wrapper_stats = Arc::new(wrapper_stats);
            clt_r.reset_stats(wrapper_stats.clone());
            clt_w.reset_stats(wrapper_stats);

            let limit_config = user_ctx
                .user_config()
                .tcp_sock_speed_limit
                .shrink_as_smaller(&self.ctx.server_config.tcp_sock_speed_limit);
            clt_r.reset_local_limit(limit_config.shift_millis, limit_config.max_north);
            clt_w.reset_local_limit(limit_config.shift_millis, limit_config.max_south);

            let user = user_ctx.user();
            if let Some(limiter) = user.tcp_all_upload_speed_limit() {
                clt_r.add_global_limiter(limiter.clone());
            }
            if let Some(limiter) = user.tcp_all_download_speed_limit() {
                clt_w.add_global_limiter(limiter.clone());
            }
        } else {
            let wrapper_stats = Arc::new(wrapper_stats);
            clt_r.reset_stats(wrapper_stats.clone());
            clt_w.reset_stats(wrapper_stats);
        }
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
        self.task_notes.user_ctx().map(|ctx| ctx.user().as_ref())
    }
}
