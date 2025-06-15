/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use http::Version;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_daemon::server::ServerQuitPolicy;
use g3_daemon::stat::task::TcpStreamTaskStats;
use g3_io_ext::{IdleInterval, LimitedReader, LimitedWriter, StreamCopyConfig};
use g3_types::acl::AclAction;
use g3_types::net::{ProxyRequestType, UpstreamAddr};

use super::protocol::{HttpClientWriter, HttpProxyRequest};
use super::{CommonTaskContext, TcpConnectTaskCltWrapperStats};
use crate::audit::AuditContext;
use crate::auth::User;
use crate::config::server::ServerConfig;
use crate::inspect::{StreamInspectContext, StreamTransitTask};
use crate::log::task::tcp_connect::TaskLogForTcpConnect;
use crate::module::http_forward::HttpProxyClientResponse;
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectTaskConf, TcpConnectTaskNotes, TcpConnection,
};
use crate::serve::{
    ServerStats, ServerTaskError, ServerTaskForbiddenError, ServerTaskNotes, ServerTaskResult,
    ServerTaskStage,
};

pub(crate) struct HttpProxyConnectTask {
    ctx: Arc<CommonTaskContext>,
    upstream: UpstreamAddr,
    stream_ups: Option<TcpConnection>,
    back_to_http: bool,
    task_notes: ServerTaskNotes,
    tcp_notes: TcpConnectTaskNotes,
    task_stats: Arc<TcpStreamTaskStats>,
    audit_ctx: AuditContext,
    http_version: Version,
    started: bool,
}

impl Drop for HttpProxyConnectTask {
    fn drop(&mut self) {
        if self.started {
            self.post_stop();
            self.started = false;
        }
    }
}

impl HttpProxyConnectTask {
    pub(crate) fn new(
        ctx: &Arc<CommonTaskContext>,
        audit_ctx: AuditContext,
        req: &HttpProxyRequest<impl AsyncRead>,
        task_notes: ServerTaskNotes,
    ) -> Self {
        HttpProxyConnectTask {
            ctx: Arc::clone(ctx),
            upstream: req.upstream.clone(),
            stream_ups: None,
            back_to_http: false,
            task_notes,
            tcp_notes: TcpConnectTaskNotes::default(),
            task_stats: Arc::new(TcpStreamTaskStats::default()),
            audit_ctx,
            http_version: req.inner.version,
            started: false,
        }
    }

    async fn reply_too_many_requests<W>(&mut self, clt_w: &mut W)
    where
        W: AsyncWrite + Unpin,
    {
        let rsp = HttpProxyClientResponse::too_many_requests(self.http_version);
        // no custom header is set
        let _ = rsp.reply_err_to_request(clt_w).await;
        self.back_to_http = false;
    }

    async fn reply_forbidden<W>(&mut self, clt_w: &mut W)
    where
        W: AsyncWrite + Unpin,
    {
        let rsp = HttpProxyClientResponse::forbidden(self.http_version);
        // no custom header is set
        let _ = rsp.reply_err_to_request(clt_w).await;
        self.back_to_http = false;
    }

    async fn reply_banned_protocol<W>(&mut self, clt_w: &mut W)
    where
        W: AsyncWrite + Unpin,
    {
        let rsp = HttpProxyClientResponse::method_not_allowed(self.http_version);
        // no custom header is set
        let _ = rsp.reply_err_to_request(clt_w).await;
        self.back_to_http = false;
    }

    async fn reply_ok<W>(&self, clt_w: &mut W) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut rsp =
            HttpProxyClientResponse::from_standard(http::StatusCode::OK, self.http_version, false);
        self.ctx
            .set_custom_header_for_local_reply(&self.tcp_notes, &mut rsp);
        rsp.reply_ok_to_connect(clt_w)
            .await
            .map_err(ServerTaskError::ClientTcpWriteFailed)
    }

    async fn reply_connect_err<W>(&mut self, e: &TcpConnectError, clt_w: &mut W)
    where
        W: AsyncWrite + Unpin,
    {
        let mut rsp =
            HttpProxyClientResponse::from_tcp_connect_error(e, http::Version::HTTP_11, false);
        self.ctx
            .set_custom_header_for_local_reply(&self.tcp_notes, &mut rsp);
        let should_close = rsp.should_close();
        self.back_to_http = !should_close;

        if rsp.reply_err_to_request(clt_w).await.is_err() {
            self.back_to_http = false;
        }
    }

    pub(crate) async fn connect_to_upstream<W>(&mut self, clt_w: &mut W)
    where
        W: AsyncWrite + Unpin,
    {
        self.pre_start();
        match self.run_connect(clt_w).await {
            Ok(()) => {
                self.back_to_http = false;
            }
            Err(e) => {
                if let Some(log_ctx) = self.get_log_context() {
                    log_ctx.log(e);
                }
            }
        }
    }

    async fn handle_server_upstream_acl_action<W>(
        &mut self,
        action: AclAction,
        clt_w: &mut W,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
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
            self.ctx.server_stats.forbidden.add_dest_denied();
            if let Some(user_ctx) = self.task_notes.user_ctx() {
                // also add to user level forbidden stats
                user_ctx.add_dest_denied();
            }

            self.reply_forbidden(clt_w).await;
            Err(ServerTaskError::ForbiddenByRule(
                ServerTaskForbiddenError::DestDenied,
            ))
        } else {
            Ok(())
        }
    }

    async fn handle_user_upstream_acl_action<W>(
        &mut self,
        action: AclAction,
        clt_w: &mut W,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
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
            self.reply_forbidden(clt_w).await;
            Err(ServerTaskError::ForbiddenByRule(
                ServerTaskForbiddenError::DestDenied,
            ))
        } else {
            Ok(())
        }
    }

    async fn handle_user_protocol_acl_action<W>(
        &mut self,
        action: AclAction,
        clt_w: &mut W,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
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
            self.reply_banned_protocol(clt_w).await;
            Err(ServerTaskError::ForbiddenByRule(
                ServerTaskForbiddenError::ProtoBanned,
            ))
        } else {
            Ok(())
        }
    }

    async fn run_connect<W>(&mut self, clt_w: &mut W) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut tcp_client_misc_opts = self.ctx.server_config.tcp_misc_opts;

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            let user_ctx = user_ctx.clone();

            if user_ctx.check_rate_limit().is_err() {
                self.reply_too_many_requests(clt_w).await;
                return Err(ServerTaskError::ForbiddenByRule(
                    ServerTaskForbiddenError::RateLimited,
                ));
            }

            match user_ctx.acquire_request_semaphore() {
                Ok(permit) => self.task_notes.user_req_alive_permit = Some(permit),
                Err(_) => {
                    self.reply_too_many_requests(clt_w).await;
                    return Err(ServerTaskError::ForbiddenByRule(
                        ServerTaskForbiddenError::FullyLoaded,
                    ));
                }
            }

            let action = user_ctx.check_proxy_request(ProxyRequestType::HttpConnect);
            self.handle_user_protocol_acl_action(action, clt_w).await?;

            let action = user_ctx.check_upstream(&self.upstream);
            self.handle_user_upstream_acl_action(action, clt_w).await?;

            tcp_client_misc_opts = user_ctx
                .user_config()
                .tcp_client_misc_opts(&tcp_client_misc_opts);
        }

        // server level dst host/port acl rules
        let action = self.ctx.check_upstream(&self.upstream);
        self.handle_server_upstream_acl_action(action, clt_w)
            .await?;

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
        match self
            .ctx
            .escaper
            .tcp_setup_connection(
                &task_conf,
                &mut self.tcp_notes,
                &self.task_notes,
                self.task_stats.clone(),
                &mut self.audit_ctx,
            )
            .await
        {
            Ok(connection) => {
                self.task_notes.stage = ServerTaskStage::Connected;
                self.stream_ups = Some(connection);
                Ok(())
            }
            Err(e) => {
                self.reply_connect_err(&e, clt_w).await;
                Err(e.into())
            }
        }
    }

    pub(crate) fn back_to_http(&self) -> bool {
        self.back_to_http
    }

    fn pre_start(&mut self) {
        self.ctx.server_stats.task_http_connect.add_task();
        self.ctx.server_stats.task_http_connect.inc_alive_task();

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.foreach_req_stats(|s| {
                s.req_total.add_http_connect();
                s.req_alive.add_http_connect();
            });
        }

        if self.ctx.server_config.flush_task_log_on_created {
            if let Some(log_ctx) = self.get_log_context() {
                log_ctx.log_created();
            }
        }

        self.started = true;
    }

    fn post_stop(&mut self) {
        self.ctx.server_stats.task_http_connect.dec_alive_task();

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.foreach_req_stats(|s| {
                s.req_alive.del_http_connect();
            });

            if let Some(user_req_alive_permit) = self.task_notes.user_req_alive_permit.take() {
                drop(user_req_alive_permit);
            }
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

    pub(crate) fn into_running<CDR, CDW>(mut self, clt_r: CDR, clt_w: HttpClientWriter<CDW>)
    where
        CDR: AsyncRead + Send + Sync + Unpin + 'static,
        CDW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        if self.stream_ups.is_none() {
            return;
        }

        tokio::spawn(async move {
            match self.stream_ups.take() {
                Some((ups_r, ups_w)) => {
                    let e = match self.run_connected(clt_r, clt_w, ups_r, ups_w).await {
                        Ok(_) => ServerTaskError::Finished,
                        Err(e) => e,
                    };
                    if let Some(log_ctx) = self.get_log_context() {
                        log_ctx.log(e);
                    }
                }
                None => unreachable!(),
            }
        });
    }

    async fn run_connected<CDR, CDW, UR, UW>(
        &mut self,
        clt_r: CDR,
        mut clt_w: HttpClientWriter<CDW>,
        ups_r: UR,
        ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        CDR: AsyncRead + Send + Sync + Unpin + 'static,
        CDW: AsyncWrite + Send + Sync + Unpin + 'static,
        UR: AsyncRead + Send + Sync + Unpin + 'static,
        UW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        if self.ctx.server_config.flush_task_log_on_connected {
            if let Some(log_ctx) = self.get_log_context() {
                log_ctx.log_connected();
            }
        }

        self.task_notes.stage = ServerTaskStage::Replying;
        self.reply_ok(&mut clt_w).await?;

        self.task_notes.mark_relaying();
        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.foreach_req_stats(|s| {
                s.req_ready.add_http_connect();
            });
        }
        let clt_w = clt_w.into_inner();
        self.relay(clt_r, clt_w, ups_r, ups_w).await
    }

    async fn relay<CDR, CDW, UR, UW>(
        &mut self,
        clt_r: CDR,
        clt_w: CDW,
        ups_r: UR,
        ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        CDR: AsyncRead + Send + Sync + Unpin + 'static,
        CDW: AsyncWrite + Send + Sync + Unpin + 'static,
        UR: AsyncRead + Send + Sync + Unpin + 'static,
        UW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let (clt_r, clt_w) = self.update_clt(clt_r, clt_w);

        if let Some(audit_handle) = self.audit_ctx.handle() {
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
                    audit_handle.clone(),
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

    fn update_clt<CDR, CDW>(
        &self,
        clt_r: CDR,
        clt_w: CDW,
    ) -> (LimitedReader<CDR>, LimitedWriter<CDW>)
    where
        CDR: AsyncRead + Unpin,
        CDW: AsyncWrite + Unpin,
    {
        let mut wrapper_stats =
            TcpConnectTaskCltWrapperStats::new(&self.ctx.server_stats, &self.task_stats);

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

impl StreamTransitTask for HttpProxyConnectTask {
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
