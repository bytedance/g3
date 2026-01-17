/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;

use g3_daemon::server::ServerQuitPolicy;
use g3_daemon::stat::task::TcpStreamTaskStats;
use g3_io_ext::{AsyncStream, IdleInterval, LimitedReader, LimitedWriter, StreamCopyConfig};
use g3_types::acl::AclAction;
use g3_types::net::UpstreamAddr;

use super::common::CommonTaskContext;
use crate::audit::AuditContext;
use crate::auth::User;
use crate::config::server::ServerConfig;
use crate::inspect::{StreamInspectContext, StreamTransitTask};
use crate::log::task::tcp_connect::TaskLogForTcpConnect;
use crate::module::tcp_connect::{TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf};
use crate::serve::tcp_stream::{TcpStreamServerAliveTaskGuard, TcpStreamTaskCltWrapperStats};
use crate::serve::{
    ServerStats, ServerTaskError, ServerTaskForbiddenError, ServerTaskNotes, ServerTaskResult,
    ServerTaskStage,
};

pub(super) struct TlsStreamTask {
    ctx: CommonTaskContext,
    upstream: UpstreamAddr,
    tcp_notes: TcpConnectTaskNotes,
    task_notes: ServerTaskNotes,
    task_stats: Arc<TcpStreamTaskStats>,
    audit_ctx: AuditContext,
    started: bool,
    _alive_guard: Option<TcpStreamServerAliveTaskGuard>,
}

impl Drop for TlsStreamTask {
    fn drop(&mut self) {
        if self.started {
            self.post_stop();
            self.started = false;
        }
    }
}

impl TlsStreamTask {
    pub(super) fn new(
        ctx: CommonTaskContext,
        upstream: &UpstreamAddr,
        audit_ctx: AuditContext,
        task_notes: ServerTaskNotes,
    ) -> Self {
        TlsStreamTask {
            ctx,
            upstream: upstream.clone(),
            tcp_notes: TcpConnectTaskNotes::default(),
            task_notes,
            task_stats: Arc::new(TcpStreamTaskStats::default()),
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

    pub(super) async fn into_running(mut self, stream: TlsStream<TcpStream>) {
        self.pre_start();
        let e = match self.run(stream).await {
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

    async fn run(&mut self, clt_stream: TlsStream<TcpStream>) -> ServerTaskResult<()> {
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
        self.run_connected(clt_stream, ups_r, ups_w).await
    }

    async fn run_connected<R, W>(
        &mut self,
        clt_stream: TlsStream<TcpStream>,
        ups_r: R,
        ups_w: W,
    ) -> ServerTaskResult<()>
    where
        R: AsyncRead + Send + Sync + Unpin + 'static,
        W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        if self.ctx.server_config.flush_task_log_on_connected
            && let Some(log_ctx) = self.get_log_context()
        {
            log_ctx.log_connected();
        }
        self.task_notes.mark_relaying();
        self.relay(clt_stream, ups_r, ups_w).await
    }

    async fn relay<R, W>(
        &mut self,
        clt_stream: TlsStream<TcpStream>,
        ups_r: R,
        ups_w: W,
    ) -> ServerTaskResult<()>
    where
        R: AsyncRead + Send + Sync + Unpin + 'static,
        W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let (clt_r, clt_w) = self.split_clt(clt_stream);

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
                return crate::inspect::stream::transit_with_inspection(
                    clt_r,
                    clt_w,
                    ups_r,
                    ups_w,
                    ctx,
                    self.upstream.clone(),
                    None,
                )
                .await;
            }
        }

        self.transit_transparent(clt_r, clt_w, ups_r, ups_w).await
    }

    fn split_clt(
        &self,
        clt_stream: TlsStream<TcpStream>,
    ) -> (
        LimitedReader<impl AsyncRead + use<>>,
        LimitedWriter<impl AsyncWrite + use<>>,
    ) {
        let (clt_r, clt_w) = clt_stream.into_split();

        let mut wrapper_stats =
            TcpStreamTaskCltWrapperStats::new(&self.ctx.server_stats, &self.task_stats);

        let limit_config = if let Some(user_ctx) = self.task_notes.user_ctx() {
            wrapper_stats.push_user_io_stats(user_ctx.fetch_traffic_stats(
                self.ctx.server_config.name(),
                self.ctx.server_stats.share_extra_tags(),
            ));

            user_ctx
                .user_config()
                .tcp_sock_speed_limit
                .shrink_as_smaller(&self.ctx.server_config.tcp_sock_speed_limit)
        } else {
            self.ctx.server_config.tcp_sock_speed_limit
        };

        let wrapper_stats = Arc::new(wrapper_stats);
        let mut clt_r = LimitedReader::local_limited(
            clt_r,
            limit_config.shift_millis,
            limit_config.max_north,
            wrapper_stats.clone(),
        );
        let mut clt_w = LimitedWriter::local_limited(
            clt_w,
            limit_config.shift_millis,
            limit_config.max_south,
            wrapper_stats,
        );

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            let user = user_ctx.user();
            if let Some(limiter) = user.tcp_all_upload_speed_limit() {
                clt_r.add_global_limiter(limiter.clone());
            }
            if let Some(limiter) = user.tcp_all_download_speed_limit() {
                clt_w.add_global_limiter(limiter.clone());
            }
        }

        (clt_r, clt_w)
    }
}

impl StreamTransitTask for TlsStreamTask {
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
