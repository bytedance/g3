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

use anyhow::anyhow;
use std::sync::Arc;

use futures_util::FutureExt;
use http::HeaderMap;
use log::debug;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::time::Instant;

use g3_http::client::HttpForwardRemoteResponse;
use g3_http::server::HttpProxyClientRequest;
use g3_http::{HttpBodyReader, HttpBodyType};
use g3_icap_client::reqmod::h1::{
    HttpAdapterErrorResponse, HttpRequestAdapter, ReqmodAdaptationEndState,
    ReqmodAdaptationRunState, ReqmodRecvHttpResponseBody,
};
use g3_icap_client::respmod::h1::{
    HttpResponseAdapter, RespmodAdaptationEndState, RespmodAdaptationRunState,
};
use g3_io_ext::{LimitedBufReadExt, LimitedCopy, LimitedCopyError};
use g3_types::acl::AclAction;
use g3_types::net::ProxyRequestType;

use super::protocol::{HttpClientReader, HttpClientWriter, HttpProxyRequest};
use super::{
    CommonTaskContext, HttpForwardTaskCltWrapperStats, HttpForwardTaskStats,
    HttpsForwardTaskCltWrapperStats,
};
use crate::config::server::ServerConfig;
use crate::log::task::http_forward::TaskLogForHttpForward;
use crate::module::http_forward::{
    BoxHttpForwardConnection, BoxHttpForwardContext, BoxHttpForwardReader, BoxHttpForwardWriter,
    HttpForwardTaskNotes, HttpProxyClientResponse,
};
use crate::module::http_header;
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskNotes};
use crate::serve::{
    ServerIdleChecker, ServerStats, ServerTaskError, ServerTaskForbiddenError, ServerTaskNotes,
    ServerTaskResult, ServerTaskStage,
};

pub(crate) struct HttpProxyForwardTask<'a> {
    ctx: Arc<CommonTaskContext>,
    req: &'a HttpProxyClientRequest,
    is_https: bool,
    should_close: bool,
    send_error_response: bool,
    task_notes: ServerTaskNotes,
    http_notes: HttpForwardTaskNotes,
    tcp_notes: TcpConnectTaskNotes,
    task_stats: Arc<HttpForwardTaskStats>,
    do_application_audit: bool,
}

impl<'a> HttpProxyForwardTask<'a> {
    pub(crate) fn new(
        ctx: &Arc<CommonTaskContext>,
        req: &'a HttpProxyRequest<impl AsyncRead>,
        is_https: bool,
        task_notes: ServerTaskNotes,
    ) -> Self {
        let mut uri_log_max_chars = ctx.server_config.log_uri_max_chars;
        let mut do_application_audit = false;
        if let Some(user_ctx) = task_notes.user_ctx() {
            let user_config = &user_ctx.user().config;
            if let Some(max_chars) = user_config.log_uri_max_chars {
                uri_log_max_chars = max_chars; // overwrite
            }
            if let Some(audit_handle) = &ctx.audit_handle {
                do_application_audit = user_config
                    .audit
                    .do_application_audit()
                    .unwrap_or_else(|| audit_handle.do_application_audit());
            }
        } else if let Some(audit_handle) = &ctx.audit_handle {
            do_application_audit = audit_handle.do_application_audit();
        }
        let http_notes = HttpForwardTaskNotes::new(
            req.time_received,
            task_notes.task_created_instant(),
            req.inner.method.clone(),
            req.inner.uri.clone(),
            uri_log_max_chars,
        );
        HttpProxyForwardTask {
            ctx: Arc::clone(ctx),
            req: &req.inner,
            is_https,
            should_close: !req.inner.keep_alive(),
            send_error_response: true,
            task_notes,
            http_notes,
            tcp_notes: TcpConnectTaskNotes::new(req.upstream.clone()),
            task_stats: Arc::new(HttpForwardTaskStats::default()),
            do_application_audit,
        }
    }

    #[inline]
    pub(crate) fn should_close(&self) -> bool {
        self.should_close
    }

    async fn reply_too_many_requests<W>(&mut self, clt_w: &mut W)
    where
        W: AsyncWrite + Unpin,
    {
        let rsp = HttpProxyClientResponse::too_many_requests(self.req.version);
        // no custom header is set
        if rsp.reply_err_to_request(clt_w).await.is_ok() {
            self.http_notes.rsp_status = rsp.status();
        }
        self.should_close = true;
    }

    async fn reply_forbidden<W>(&mut self, clt_w: &mut W)
    where
        W: AsyncWrite + Unpin,
    {
        let rsp = HttpProxyClientResponse::forbidden(self.req.version);
        // no custom header is set
        if rsp.reply_err_to_request(clt_w).await.is_ok() {
            self.http_notes.rsp_status = rsp.status();
        }
        self.should_close = true;
    }

    async fn reply_banned_protocol<W>(&mut self, clt_w: &mut W)
    where
        W: AsyncWrite + Unpin,
    {
        let rsp = HttpProxyClientResponse::method_not_allowed(self.req.version);
        // no custom header is set
        if rsp.reply_err_to_request(clt_w).await.is_ok() {
            self.http_notes.rsp_status = rsp.status();
        }
        self.should_close = true;
    }

    async fn reply_connect_err<W>(&mut self, e: &TcpConnectError, clt_w: &mut W)
    where
        W: AsyncWrite + Unpin,
    {
        let mut rsp = HttpProxyClientResponse::from_tcp_connect_error(
            e,
            self.req.version,
            self.should_close || self.req.body_type().is_some(),
        );

        self.ctx
            .set_custom_header_for_local_reply(&self.tcp_notes, &mut rsp);

        if rsp.should_close() {
            self.should_close = true;
        }

        if rsp.reply_err_to_request(clt_w).await.is_err() {
            self.should_close = true;
        } else {
            self.http_notes.rsp_status = rsp.status();
        }
    }

    async fn reply_task_err<W>(&mut self, e: &ServerTaskError, clt_w: &mut W)
    where
        W: AsyncWrite + Unpin,
    {
        let body_pending = self.req.body_type().is_some();
        let rsp = HttpProxyClientResponse::from_task_err(
            e,
            self.req.version,
            self.should_close || body_pending,
        );

        if let Some(mut rsp) = rsp {
            self.ctx
                .set_custom_header_for_local_reply(&self.tcp_notes, &mut rsp);

            if rsp.should_close() {
                self.should_close = true;
            }

            if rsp.reply_err_to_request(clt_w).await.is_err() {
                self.should_close = true;
            } else {
                self.http_notes.rsp_status = rsp.status();
            }
        } else if body_pending {
            self.should_close = true;
        }
    }

    fn get_log_context(&self) -> TaskLogForHttpForward {
        let http_user_agent = self
            .req
            .end_to_end_headers
            .get(http::header::USER_AGENT)
            .map(|v| v.to_str().unwrap_or("invalid"));
        TaskLogForHttpForward {
            task_notes: &self.task_notes,
            http_notes: &self.http_notes,
            http_user_agent,
            tcp_notes: &self.tcp_notes,
            total_time: self.task_notes.time_elapsed(),
            client_rd_bytes: self.task_stats.clt.read.get_bytes(),
            client_wr_bytes: self.task_stats.clt.write.get_bytes(),
            remote_rd_bytes: self.task_stats.ups.read.get_bytes(),
            remote_wr_bytes: self.task_stats.ups.write.get_bytes(),
        }
    }

    pub(crate) async fn run<CDR, CDW>(
        &mut self,
        clt_r: &mut Option<HttpClientReader<CDR>>,
        clt_w: &mut HttpClientWriter<CDW>,
        fwd_ctx: &mut BoxHttpForwardContext,
    ) where
        CDR: AsyncRead + Send + Unpin,
        CDW: AsyncWrite + Send + Unpin,
    {
        self.pre_start();
        match self.run_forward(clt_r, clt_w, fwd_ctx).await {
            Ok(()) => {
                self.get_log_context()
                    .log(&self.ctx.task_logger, &ServerTaskError::Finished);
            }
            Err(e) => {
                self.get_log_context().log(&self.ctx.task_logger, &e);
            }
        }
        self.pre_stop();
    }

    fn pre_start(&self) {
        debug!(
            "HttpProxy/FORWARD: new client from {} to {} server {}, using escaper {}",
            self.ctx.tcp_client_addr,
            self.ctx.server_config.server_type(),
            self.ctx.server_config.name(),
            self.ctx.server_config.escaper
        );
        self.ctx.server_stats.task_http_forward.add_task();
        self.ctx.server_stats.task_http_forward.inc_alive_task();

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx
                .req_stats()
                .req_total
                .add_http_forward(self.is_https);
            user_ctx
                .req_stats()
                .req_alive
                .add_http_forward(self.is_https);

            if let Some(site_req_stats) = user_ctx.site_req_stats() {
                site_req_stats.req_total.add_http_forward(self.is_https);
                site_req_stats.req_alive.add_http_forward(self.is_https);
            }
        }
    }

    fn pre_stop(&mut self) {
        self.ctx.server_stats.task_http_forward.dec_alive_task();

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx
                .req_stats()
                .req_alive
                .del_http_forward(self.is_https);

            if let Some(site_req_stats) = user_ctx.site_req_stats() {
                site_req_stats.req_alive.del_http_forward(self.is_https);
            }

            if let Some(user_req_alive_permit) = self.task_notes.user_req_alive_permit.take() {
                drop(user_req_alive_permit);
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

    async fn handle_user_ua_acl_action<W>(
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
                ServerTaskForbiddenError::UaBlocked,
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

    fn setup_clt_limit_and_stats<CDR, CDW>(
        &mut self,
        clt_r: &mut Option<HttpClientReader<CDR>>,
        clt_w: &mut HttpClientWriter<CDW>,
    ) where
        CDR: AsyncRead + Unpin,
        CDW: AsyncWrite + Unpin,
    {
        let origin_header_size = self.req.origin_header_size() as u64;
        self.task_stats.clt.read.add_bytes(origin_header_size);

        let (clt_r_stats, clt_w_stats, limit_config) = if self.is_https {
            let mut wrapper_stats =
                HttpsForwardTaskCltWrapperStats::new(&self.ctx.server_stats, &self.task_stats);

            let limit_config = if let Some(user_ctx) = self.task_notes.user_ctx() {
                let user_io_stats = user_ctx.fetch_traffic_stats(
                    self.ctx.server_config.name(),
                    self.ctx.server_stats.extra_tags(),
                );
                for s in &user_io_stats {
                    s.io.https_forward.add_in_bytes(origin_header_size);
                }
                wrapper_stats.push_user_io_stats(user_io_stats);

                let user = user_ctx.user();
                if user
                    .config
                    .tcp_sock_speed_limit
                    .eq(&self.ctx.server_config.tcp_sock_speed_limit)
                {
                    None
                } else {
                    let limit_config = user
                        .config
                        .tcp_sock_speed_limit
                        .shrink_as_smaller(&self.ctx.server_config.tcp_sock_speed_limit);
                    Some(limit_config)
                }
            } else {
                None
            };

            let (clt_r_stats, clt_w_stats) = wrapper_stats.split();
            (clt_r_stats, clt_w_stats, limit_config)
        } else {
            let mut wrapper_stats =
                HttpForwardTaskCltWrapperStats::new(&self.ctx.server_stats, &self.task_stats);

            let limit_config = if let Some(user_ctx) = self.task_notes.user_ctx() {
                let user_io_stats = user_ctx.fetch_traffic_stats(
                    self.ctx.server_config.name(),
                    self.ctx.server_stats.extra_tags(),
                );
                for s in &user_io_stats {
                    s.io.http_forward.add_in_bytes(origin_header_size);
                }
                wrapper_stats.push_user_io_stats(user_io_stats);

                let user = user_ctx.user();
                if user
                    .config
                    .tcp_sock_speed_limit
                    .eq(&self.ctx.server_config.tcp_sock_speed_limit)
                {
                    None
                } else {
                    let limit_config = user
                        .config
                        .tcp_sock_speed_limit
                        .shrink_as_smaller(&self.ctx.server_config.tcp_sock_speed_limit);
                    Some(limit_config)
                }
            } else {
                None
            };

            let (clt_r_stats, clt_w_stats) = wrapper_stats.split();
            (clt_r_stats, clt_w_stats, limit_config)
        };

        if let Some(br) = clt_r {
            br.reset_buffer_stats(clt_r_stats);
            clt_w.reset_stats(clt_w_stats);
            if let Some(limit_config) = &limit_config {
                br.reset_limit(limit_config.shift_millis, limit_config.max_north);
                clt_w.reset_limit(limit_config.shift_millis, limit_config.max_south);
            }
        } else {
            clt_w.reset_stats(clt_w_stats);
            if let Some(limit_config) = &limit_config {
                clt_w.reset_limit(limit_config.shift_millis, limit_config.max_south);
            }
        }
    }

    async fn run_forward<CDR, CDW>(
        &mut self,
        clt_r: &mut Option<HttpClientReader<CDR>>,
        clt_w: &mut HttpClientWriter<CDW>,
        fwd_ctx: &mut BoxHttpForwardContext,
    ) -> ServerTaskResult<()>
    where
        CDR: AsyncRead + Send + Unpin,
        CDW: AsyncWrite + Send + Unpin,
    {
        let mut upstream_keepalive = self.ctx.server_config.http_forward_upstream_keepalive;
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

            let request_type = if self.is_https {
                ProxyRequestType::HttpsForward
            } else {
                ProxyRequestType::HttpForward
            };
            let action = user_ctx.check_proxy_request(request_type);
            self.handle_user_protocol_acl_action(action, clt_w).await?;

            let action = user_ctx.check_upstream(&self.tcp_notes.upstream);
            self.handle_user_upstream_acl_action(action, clt_w).await?;

            if let Some(action) = user_ctx.check_http_user_agent(&self.req.end_to_end_headers) {
                self.handle_user_ua_acl_action(action, clt_w).await?;
            }

            upstream_keepalive =
                upstream_keepalive.adjust_to(user_ctx.user().config.http_upstream_keepalive);
            tcp_client_misc_opts = user_ctx
                .user()
                .config
                .tcp_client_misc_opts(&tcp_client_misc_opts);
        }

        // server level dst host/port acl rules
        let action = self.ctx.check_upstream(&self.tcp_notes.upstream);
        self.handle_server_upstream_acl_action(action, clt_w)
            .await?;

        // set client side socket options
        g3_socket::tcp::set_raw_opts(self.ctx.tcp_client_socket, &tcp_client_misc_opts, true)
            .map_err(|_| {
                ServerTaskError::InternalServerError("failed to set client socket options")
            })?;

        self.setup_clt_limit_and_stats(clt_r, clt_w);

        fwd_ctx.prepare_connection(&self.tcp_notes.upstream, self.is_https);

        if let Some(connection) = fwd_ctx
            .get_alive_connection(
                &self.task_notes,
                self.task_stats.for_escaper(),
                upstream_keepalive.idle_expire(),
            )
            .await
        {
            self.task_notes.stage = ServerTaskStage::Connected;
            self.http_notes.reuse_connection = true;
            fwd_ctx.fetch_tcp_notes(&mut self.tcp_notes);
            self.http_notes.retry_new_connection = true;
            if let Some(user_ctx) = self.task_notes.user_ctx() {
                user_ctx
                    .req_stats()
                    .req_reuse
                    .add_http_forward(self.is_https);
                if let Some(site_req_stats) = user_ctx.site_req_stats() {
                    site_req_stats.req_reuse.add_http_forward(self.is_https);
                }
            }

            let r = self
                .run_with_connection(clt_r, clt_w, connection, true)
                .await;
            match r {
                Ok(r) => {
                    if let Some(connection) = r {
                        fwd_ctx.save_alive_connection(connection);
                    }
                    return Ok(());
                }
                Err(e) => {
                    if self.http_notes.retry_new_connection {
                        // continue to make new connection
                        if let Some(user_ctx) = self.task_notes.user_ctx() {
                            user_ctx
                                .req_stats()
                                .req_renew
                                .add_http_forward(self.is_https);
                            if let Some(site_req_stats) = user_ctx.site_req_stats() {
                                site_req_stats.req_renew.add_http_forward(self.is_https);
                            }
                        }
                    } else {
                        self.should_close = true;
                        if self.send_error_response {
                            self.reply_task_err(&e, clt_w).await;
                        }
                        return Err(e);
                    }
                }
            }
        }

        self.task_notes.stage = ServerTaskStage::Connecting;
        self.http_notes.reuse_connection = false;
        match self.make_new_connection(fwd_ctx).await {
            Ok(connection) => {
                self.task_notes.stage = ServerTaskStage::Connected;
                fwd_ctx.fetch_tcp_notes(&mut self.tcp_notes);

                let r = self
                    .run_with_connection(clt_r, clt_w, connection, false)
                    .await;
                // handle result
                match r {
                    Ok(r) => {
                        if let Some(connection) = r {
                            fwd_ctx.save_alive_connection(connection);
                        }
                        Ok(())
                    }
                    Err(e) => {
                        self.should_close = true;
                        if self.send_error_response {
                            self.reply_task_err(&e, clt_w).await;
                        }
                        Err(e)
                    }
                }
            }
            Err(e) => {
                fwd_ctx.fetch_tcp_notes(&mut self.tcp_notes);
                self.should_close = true;
                self.reply_connect_err(&e, clt_w).await;
                Err(e.into())
            }
        }
    }

    async fn make_new_connection(
        &self,
        fwd_ctx: &mut BoxHttpForwardContext,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        if self.is_https {
            let tls_name = self
                .req
                .host
                .as_ref()
                .unwrap_or(&self.tcp_notes.upstream)
                .host_str();

            fwd_ctx
                .make_new_https_connection(
                    &self.task_notes,
                    self.task_stats.for_escaper(),
                    &self.ctx.tls_client_config,
                    &tls_name,
                )
                .await
        } else {
            fwd_ctx
                .make_new_http_connection(&self.task_notes, self.task_stats.for_escaper())
                .await
        }
    }

    fn mark_relaying(&mut self) {
        self.task_notes.mark_relaying();
        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx
                .req_stats()
                .req_ready
                .add_http_forward(self.is_https);
            if let Some(site_req_stats) = user_ctx.site_req_stats() {
                site_req_stats.req_ready.add_http_forward(self.is_https);
            }
        }
    }

    async fn run_with_connection<'f, CDR, CDW>(
        &'f mut self,
        clt_r: &'f mut Option<HttpClientReader<CDR>>,
        clt_w: &'f mut HttpClientWriter<CDW>,
        mut ups_c: BoxHttpForwardConnection,
        reused_connection: bool,
    ) -> ServerTaskResult<Option<BoxHttpForwardConnection>>
    where
        CDR: AsyncRead + Send + Unpin,
        CDW: AsyncWrite + Send + Unpin,
    {
        if reused_connection {
            if let Some(r) = ups_c.1.fill_wait_eof().now_or_never() {
                return match r {
                    Ok(_) => Err(ServerTaskError::ClosedByUpstream),
                    Err(e) => Err(ServerTaskError::UpstreamReadFailed(e)),
                };
            }
        }
        ups_c
            .0
            .prepare_new(&self.task_notes, &self.tcp_notes.upstream);

        if self.do_application_audit {
            if let Some(audit_handle) = &self.ctx.audit_handle {
                if let Some(reqmod) = audit_handle.icap_reqmod_client() {
                    match reqmod
                        .h1_adapter(
                            self.ctx.server_config.tcp_copy,
                            self.ctx.server_config.body_line_max_len,
                            true,
                            self.ctx.idle_checker(&self.task_notes),
                        )
                        .await
                    {
                        Ok(mut adapter) => {
                            let mut adaptation_state = ReqmodAdaptationRunState::new(
                                self.task_notes.task_created_instant(),
                            );
                            adapter.set_client_addr(self.ctx.tcp_client_addr);
                            if let Some(user_ctx) = self.task_notes.user_ctx() {
                                adapter.set_client_username(user_ctx.user().name());
                            }
                            let r = self
                                .run_with_adaptation(
                                    clt_r,
                                    clt_w,
                                    ups_c,
                                    adapter,
                                    &mut adaptation_state,
                                )
                                .await;
                            if let Some(dur) = adaptation_state.dur_ups_send_header {
                                self.http_notes.retry_new_connection = false;
                                self.http_notes.dur_req_send_hdr = dur;
                            }
                            if let Some(dur) = adaptation_state.dur_ups_send_all {
                                self.http_notes.dur_req_send_all = dur;
                            }
                            return r;
                        }
                        Err(e) => {
                            if !reqmod.bypass() {
                                return Err(ServerTaskError::InternalAdapterError(e));
                            }
                        }
                    }
                }
            }
        }

        self.run_without_adaptation(clt_r, clt_w, ups_c).await
    }

    async fn run_without_adaptation<'f, CDR, CDW>(
        &'f mut self,
        clt_r: &'f mut Option<HttpClientReader<CDR>>,
        clt_w: &'f mut HttpClientWriter<CDW>,
        ups_c: BoxHttpForwardConnection,
    ) -> ServerTaskResult<Option<BoxHttpForwardConnection>>
    where
        CDR: AsyncRead + Unpin,
        CDW: AsyncWrite + Send + Unpin,
    {
        if self.req.body_type().is_none() {
            self.mark_relaying();
            self.run_without_body(clt_w, ups_c).await
        } else if let Some(br) = clt_r {
            self.mark_relaying();
            self.run_with_body(br, clt_w, ups_c).await
        } else {
            // there should be a body reader
            Err(ServerTaskError::InternalServerError(
                "http body is expected but no body reader supplied",
            ))
        }
    }

    async fn run_with_adaptation<'f, CDR, CDW>(
        &'f mut self,
        clt_r: &'f mut Option<HttpClientReader<CDR>>,
        clt_w: &'f mut HttpClientWriter<CDW>,
        mut ups_c: BoxHttpForwardConnection,
        icap_adapter: HttpRequestAdapter<ServerIdleChecker>,
        adaptation_state: &'f mut ReqmodAdaptationRunState,
    ) -> ServerTaskResult<Option<BoxHttpForwardConnection>>
    where
        CDR: AsyncRead + Send + Unpin,
        CDW: AsyncWrite + Send + Unpin,
    {
        use crate::module::http_forward::HttpForwardWriterForAdaptation;

        let ups_w = &mut ups_c.0;
        let ups_r = &mut ups_c.1;

        let mut ups_w_adaptation = HttpForwardWriterForAdaptation { inner: ups_w };
        let mut adaptation_fut = icap_adapter
            .xfer(
                adaptation_state,
                self.req,
                clt_r.as_mut(),
                &mut ups_w_adaptation,
            )
            .boxed();

        let mut rsp_header: Option<HttpForwardRemoteResponse> = None;
        loop {
            tokio::select! {
                biased;

                r = ups_r.fill_wait_data() => {
                    match r {
                        Ok(true) => {
                            // we got some data from upstream
                            let hdr = self.recv_response_header(ups_r).await?;
                            if hdr.code == 100 { // HTTP CONTINUE
                                self.send_response_header(clt_w, &hdr).await?;
                                // continue
                            } else {
                                rsp_header = Some(hdr);
                                break;
                            }
                        }
                        Ok(false) => return Err(ServerTaskError::ClosedByUpstream),
                        Err(e) => return Err(ServerTaskError::UpstreamReadFailed(e)),
                    }
                }
                r = &mut adaptation_fut => {
                    match r {
                        Ok(ReqmodAdaptationEndState::OriginalTransferred) => {
                            break;
                        }
                        Ok(ReqmodAdaptationEndState::AdaptedTransferred(_r)) => {
                            // TODO add log for adapted request?
                            break;
                        }
                        Ok(ReqmodAdaptationEndState::HttpErrResponse(rsp, rsp_recv_body)) => {
                            self.send_adaptation_error_response(clt_w, rsp, rsp_recv_body).await?;
                            return Ok(None);
                        }
                        Err(e) => {
                            drop(adaptation_fut);
                            if !adaptation_state.clt_read_finished {
                                // not all client data read in, drop the client connection
                                self.should_close = true;
                            }
                            return Err(e.into());
                        }
                    }
                }
            }
        }
        drop(adaptation_fut);

        let mut close_remote = false;
        let mut rsp_header = match rsp_header {
            Some(header) => {
                if !adaptation_state.clt_read_finished {
                    // not all client data read in, drop the client connection
                    self.should_close = true;
                }
                if !adaptation_state.ups_write_finished {
                    // not all client data sent out, only drop the remote connection
                    close_remote = true;
                }
                // if not all data sent to remote, the remote response should be `close`,
                // and the remote connection will close if remote has set `close`
                header
            }
            None => {
                match tokio::time::timeout(
                    self.ctx.server_config.timeout.recv_rsp_header,
                    self.recv_final_response_header(ups_r, clt_w),
                )
                .await
                {
                    Ok(Ok(rsp_header)) => rsp_header,
                    Ok(Err(e)) => return Err(e),
                    Err(_) => {
                        return Err(ServerTaskError::UpstreamAppTimeout(
                            "timeout to receive response header",
                        ))
                    }
                }
            }
        };
        self.http_notes.mark_rsp_recv_hdr();

        self.send_response(
            clt_w,
            ups_r,
            &mut rsp_header,
            adaptation_state.take_respond_shared_headers(),
        )
        .await?;

        self.task_notes.stage = ServerTaskStage::Finished;
        if self.should_close || close_remote {
            if self.is_https {
                // make sure we correctly shutdown tls connection, or the ticket won't be reused
                // FIXME use async drop at escaper side when supported
                let _ = ups_w.shutdown().await;
            }
            Ok(None)
        } else {
            Ok(Some(ups_c))
        }
    }

    async fn send_adaptation_error_response<W>(
        &mut self,
        clt_w: &mut W,
        mut rsp: HttpAdapterErrorResponse,
        rsp_recv_body: Option<ReqmodRecvHttpResponseBody>,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        self.should_close = true;

        self.ctx
            .set_custom_header_for_adaptation_error_reply(&self.tcp_notes, &mut rsp);

        let buf = rsp.serialize(self.should_close);
        self.send_error_response = false;
        clt_w
            .write_all(buf.as_ref())
            .await
            .map_err(ServerTaskError::ClientTcpWriteFailed)?;
        self.http_notes.rsp_status = rsp.status.as_u16();

        if let Some(mut recv_body) = rsp_recv_body {
            let mut body_reader = recv_body.body_reader();
            let copy_to_clt =
                LimitedCopy::new(&mut body_reader, clt_w, &self.ctx.server_config.tcp_copy);
            copy_to_clt.await.map_err(|e| match e {
                LimitedCopyError::ReadFailed(e) => ServerTaskError::InternalAdapterError(anyhow!(
                    "read http error response from adapter failed: {e:?}"
                )),
                LimitedCopyError::WriteFailed(e) => ServerTaskError::ClientTcpWriteFailed(e),
            })?;
            recv_body.save_connection().await;
        }

        Ok(())
    }

    async fn run_without_body<W>(
        &mut self,
        clt_w: &mut W,
        mut ups_c: BoxHttpForwardConnection,
    ) -> ServerTaskResult<Option<BoxHttpForwardConnection>>
    where
        W: AsyncWrite + Send + Unpin,
    {
        let ups_w = &mut ups_c.0;
        let ups_r = &mut ups_c.1;

        self.send_request_header(ups_w).await?;
        self.http_notes.mark_req_send_hdr();
        self.http_notes.mark_req_no_body();
        self.http_notes.retry_new_connection = false;

        let mut rsp_header = match tokio::time::timeout(
            self.ctx.server_config.timeout.recv_rsp_header,
            self.recv_response_header(ups_r),
        )
        .await
        {
            Ok(Ok(rsp_header)) => rsp_header,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                return Err(ServerTaskError::UpstreamAppTimeout(
                    "timeout to receive response header",
                ))
            }
        };
        self.http_notes.mark_rsp_recv_hdr();

        self.send_response(clt_w, ups_r, &mut rsp_header, None)
            .await?;

        self.task_notes.stage = ServerTaskStage::Finished;
        if self.should_close {
            if self.is_https {
                // make sure we correctly shutdown tls connection, or the ticket won't be reused
                // FIXME use async drop at escaper side when supported
                let _ = ups_w.shutdown().await;
            }
            Ok(None)
        } else {
            Ok(Some(ups_c))
        }
    }

    async fn run_with_body<R, W>(
        &mut self,
        clt_r: &mut R,
        clt_w: &mut W,
        mut ups_c: BoxHttpForwardConnection,
    ) -> ServerTaskResult<Option<BoxHttpForwardConnection>>
    where
        R: AsyncBufRead + Unpin,
        W: AsyncWrite + Send + Unpin,
    {
        let ups_w = &mut ups_c.0;
        let ups_r = &mut ups_c.1;

        self.send_request_header(ups_w).await?;
        self.http_notes.mark_req_send_hdr();
        self.http_notes.retry_new_connection = false;

        let mut clt_body_reader = HttpBodyReader::new(
            clt_r,
            self.req.body_type().unwrap(),
            self.ctx.server_config.body_line_max_len,
        );
        let mut rsp_header: Option<HttpForwardRemoteResponse> = None;

        let mut clt_to_ups = LimitedCopy::new(
            &mut clt_body_reader,
            ups_w,
            &self.ctx.server_config.tcp_copy,
        );

        let idle_duration = self.ctx.server_config.task_idle_check_duration;
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;
        loop {
            tokio::select! {
                biased;

                r = ups_r.fill_wait_data() => {
                    match r {
                        Ok(true) => {
                            // we got some data from upstream
                            let hdr = self.recv_response_header(ups_r).await?;
                            if hdr.code == 100 { // HTTP CONTINUE
                                self.send_response_header(clt_w, &hdr).await?;
                                // continue
                            } else {
                                rsp_header = Some(hdr);
                                break;
                            }
                        }
                        Ok(false) => return Err(ServerTaskError::ClosedByUpstream),
                        Err(e) => return Err(ServerTaskError::UpstreamReadFailed(e)),
                    }
                }
                r = &mut clt_to_ups => {
                    r.map_err(|e| match e {
                        LimitedCopyError::ReadFailed(e) => ServerTaskError::ClientTcpReadFailed(e),
                        LimitedCopyError::WriteFailed(e) => ServerTaskError::UpstreamWriteFailed(e),
                    })?;
                    self.http_notes.mark_req_send_all();
                    break;
                }
                _ = idle_interval.tick() => {
                    if clt_to_ups.is_idle() {
                        idle_count += 1;

                        let quit = if let Some(user_ctx) = self.task_notes.user_ctx() {
                            let user = user_ctx.user();
                            if user.is_blocked() {
                                return Err(ServerTaskError::CanceledAsUserBlocked);
                            }
                            idle_count >= user.task_max_idle_count()
                        } else {
                            idle_count >= self.ctx.server_config.task_idle_max_count
                        };

                        if quit {
                            return if clt_to_ups.no_cached_data() {
                                Err(ServerTaskError::ClientAppTimeout("idle while reading request body"))
                            } else {
                                Err(ServerTaskError::UpstreamAppTimeout("idle while sending request body"))
                            };
                        }
                    } else {
                        idle_count = 0;

                        clt_to_ups.reset_active();
                    }

                    if let Some(user_ctx) = self.task_notes.user_ctx() {
                        if user_ctx.user().is_blocked() {
                            return Err(ServerTaskError::CanceledAsUserBlocked);
                        }
                    }

                    if self.ctx.server_quit_policy.force_quit() {
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            };
        }
        drop(idle_interval);

        let mut close_remote = false;
        let copy_done = clt_to_ups.finished();
        let mut rsp_header = match rsp_header {
            Some(header) => {
                if !clt_body_reader.finished() {
                    // not all client data read in, drop the client connection
                    self.should_close = true;
                }
                if !copy_done {
                    // not all client data sent out, only drop the remote connection
                    close_remote = true;
                }
                // if not all data sent to remote, the remote response should be `close`,
                // and the remote connection will close if remote has set `close`
                header
            }
            None => {
                match tokio::time::timeout(
                    self.ctx.server_config.timeout.recv_rsp_header,
                    self.recv_final_response_header(ups_r, clt_w),
                )
                .await
                {
                    Ok(Ok(rsp_header)) => rsp_header,
                    Ok(Err(e)) => return Err(e),
                    Err(_) => {
                        return Err(ServerTaskError::UpstreamAppTimeout(
                            "timeout to receive response header",
                        ))
                    }
                }
            }
        };
        self.http_notes.mark_rsp_recv_hdr();

        self.send_response(clt_w, ups_r, &mut rsp_header, None)
            .await?;

        self.task_notes.stage = ServerTaskStage::Finished;
        if self.should_close || close_remote {
            if self.is_https {
                // make sure we correctly shutdown tls connection, or the ticket won't be reused
                // FIXME use async drop at escaper side when supported
                let _ = ups_w.shutdown().await;
            }
            Ok(None)
        } else {
            Ok(Some(ups_c))
        }
    }

    async fn recv_final_response_header<W>(
        &mut self,
        ups_r: &mut BoxHttpForwardReader,
        clt_w: &mut W,
    ) -> ServerTaskResult<HttpForwardRemoteResponse>
    where
        W: AsyncWrite + Unpin,
    {
        let hdr = self.recv_response_header(ups_r).await?;
        if hdr.code == 100 {
            // HTTP CONTINUE
            self.send_response_header(clt_w, &hdr).await?;
            // recv the final response header
            self.recv_response_header(ups_r).await
        } else {
            Ok(hdr)
        }
    }

    async fn send_request_header(&self, ups_w: &mut BoxHttpForwardWriter) -> ServerTaskResult<()> {
        ups_w
            .send_request_header(self.req)
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)?;
        ups_w
            .flush()
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)?;
        Ok(())
    }

    async fn recv_response_header(
        &mut self,
        ups_r: &mut BoxHttpForwardReader,
    ) -> ServerTaskResult<HttpForwardRemoteResponse> {
        ups_r
            .recv_response_header(
                &self.req.method,
                self.req.keep_alive(),
                self.ctx.server_config.rsp_hdr_max_size,
                &mut self.http_notes,
            )
            .await
            .map_err(|e| e.into())
    }

    async fn send_response<R, W>(
        &mut self,
        clt_w: &mut W,
        ups_r: &mut R,
        rsp_header: &mut HttpForwardRemoteResponse,
        adaptation_respond_shared_headers: Option<HeaderMap>,
    ) -> ServerTaskResult<()>
    where
        R: AsyncBufRead + Unpin,
        W: AsyncWrite + Send + Unpin,
    {
        if self.should_close {
            rsp_header.set_no_keep_alive();
        }
        if !rsp_header.keep_alive() {
            self.should_close = true;
        }
        self.http_notes.origin_status = rsp_header.code;
        self.http_notes.rsp_status = 0;
        self.update_response_header(rsp_header);

        if self.do_application_audit {
            if let Some(audit_handle) = &self.ctx.audit_handle {
                if let Some(respmod) = audit_handle.icap_respmod_client() {
                    match respmod
                        .h1_adapter(
                            self.ctx.server_config.tcp_copy,
                            self.ctx.server_config.body_line_max_len,
                            self.ctx.idle_checker(&self.task_notes),
                        )
                        .await
                    {
                        Ok(mut adapter) => {
                            let mut adaptation_state = RespmodAdaptationRunState::new(
                                self.task_notes.task_created_instant(),
                                self.http_notes.dur_rsp_recv_hdr,
                            );
                            adapter.set_client_addr(self.ctx.tcp_client_addr);
                            if let Some(user_ctx) = self.task_notes.user_ctx() {
                                adapter.set_client_username(user_ctx.user().name());
                            }
                            adapter.set_respond_shared_headers(adaptation_respond_shared_headers);
                            let r = self
                                .send_response_with_adaptation(
                                    clt_w,
                                    ups_r,
                                    rsp_header,
                                    adapter,
                                    &mut adaptation_state,
                                )
                                .await;
                            if !adaptation_state.clt_write_finished
                                || !adaptation_state.ups_read_finished
                            {
                                self.should_close = true;
                            }
                            if let Some(dur) = adaptation_state.dur_ups_recv_all {
                                self.http_notes.dur_rsp_recv_all = dur;
                            }
                            self.send_error_response = !adaptation_state.clt_write_started;
                            return r;
                        }
                        Err(e) => {
                            if !respmod.bypass() {
                                return Err(ServerTaskError::InternalAdapterError(e));
                            }
                        }
                    }
                }
            }
        }

        self.send_response_without_adaptation(clt_w, ups_r, rsp_header)
            .await
    }

    async fn send_response_with_adaptation<R, W>(
        &mut self,
        clt_w: &mut W,
        ups_r: &mut R,
        rsp_header: &HttpForwardRemoteResponse,
        icap_adapter: HttpResponseAdapter<ServerIdleChecker>,
        adaptation_state: &mut RespmodAdaptationRunState,
    ) -> ServerTaskResult<()>
    where
        R: AsyncBufRead + Unpin,
        W: AsyncWrite + Send + Unpin,
    {
        match icap_adapter
            .xfer(adaptation_state, self.req, rsp_header, ups_r, clt_w)
            .await
        {
            Ok(RespmodAdaptationEndState::OriginalTransferred) => {
                self.http_notes.rsp_status = rsp_header.code;
                Ok(())
            }
            Ok(RespmodAdaptationEndState::AdaptedTransferred(adapted_rsp)) => {
                self.http_notes.rsp_status = adapted_rsp.code;
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn send_response_without_adaptation<R, W>(
        &mut self,
        clt_w: &mut W,
        ups_r: &mut R,
        rsp_header: &HttpForwardRemoteResponse,
    ) -> ServerTaskResult<()>
    where
        R: AsyncBufRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        self.send_error_response = false;

        if let Some(body_type) = rsp_header.body_type(&self.req.method) {
            let mut buf = Vec::with_capacity(self.ctx.server_config.tcp_copy.buffer_size());
            rsp_header.serialize_to(&mut buf);
            self.http_notes.rsp_status = rsp_header.code; // the following function must send rsp header out
            self.send_response_body(buf, clt_w, ups_r, body_type).await
        } else {
            self.send_response_header(clt_w, rsp_header).await?;
            self.http_notes.rsp_status = rsp_header.code;
            self.http_notes.mark_rsp_no_body();
            Ok(())
        }
    }

    async fn send_response_body<R, W>(
        &mut self,
        header: Vec<u8>,
        clt_w: &mut W,
        ups_r: &mut R,
        body_type: HttpBodyType,
    ) -> ServerTaskResult<()>
    where
        R: AsyncBufRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let header_len = header.len() as u64;
        let mut body_reader =
            HttpBodyReader::new(ups_r, body_type, self.ctx.server_config.body_line_max_len);

        let mut ups_to_clt = LimitedCopy::with_data(
            &mut body_reader,
            clt_w,
            &self.ctx.server_config.tcp_copy,
            header,
        );

        let idle_duration = self.ctx.server_config.task_idle_check_duration;
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;
        loop {
            tokio::select! {
                biased;

                r = &mut ups_to_clt => {
                    return match r {
                        Ok(_) => {
                            self.http_notes.mark_rsp_recv_all();
                            // clt_w is already flushed
                            Ok(())
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => {
                            if ups_to_clt.copied_size() < header_len {
                                let _ = ups_to_clt.write_flush().await; // flush rsp header to client
                            }
                            Err(ServerTaskError::UpstreamReadFailed(e))
                        }
                        Err(LimitedCopyError::WriteFailed(e)) => Err(ServerTaskError::ClientTcpWriteFailed(e)),
                    };
                }
                _ = idle_interval.tick() => {
                    if ups_to_clt.is_idle() {
                        idle_count += 1;

                        let quit = if let Some(user_ctx) = self.task_notes.user_ctx() {
                            let user = user_ctx.user();
                            if user.is_blocked() {
                                if ups_to_clt.copied_size() < header_len {
                                    let _ = ups_to_clt.write_flush().await; // flush rsp header to client
                                }
                                return Err(ServerTaskError::CanceledAsUserBlocked);
                            }
                            idle_count >= user.task_max_idle_count()
                        } else {
                            idle_count >= self.ctx.server_config.task_idle_max_count
                        };

                        if quit {
                            return if ups_to_clt.no_cached_data() {
                                Err(ServerTaskError::UpstreamAppTimeout("idle while reading response body"))
                            } else {
                                Err(ServerTaskError::ClientAppTimeout("idle while sending response with body"))
                            };
                        }
                    } else {
                        idle_count = 0;

                        ups_to_clt.reset_active();
                    }

                    if let Some(user_ctx) = self.task_notes.user_ctx() {
                        if user_ctx.user().is_blocked() {
                            if ups_to_clt.copied_size() < header_len {
                                let _ = ups_to_clt.write_flush().await; // flush rsp header to client
                            }
                            return Err(ServerTaskError::CanceledAsUserBlocked);
                        }
                    }

                    if self.ctx.server_quit_policy.force_quit() {
                        if ups_to_clt.copied_size() < header_len {
                            let _ = ups_to_clt.write_flush().await; // flush rsp header to client
                        }
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            }
        }
    }

    fn update_response_header(&self, rsp: &mut HttpForwardRemoteResponse) {
        // append headers to hop-by-hop headers, so they will pass to client without adaptation
        if let Some(server_id) = &self.ctx.server_config.server_id {
            if self.ctx.server_config.http_forward_mark_upstream {
                http_header::set_upstream_id(&mut rsp.hop_by_hop_headers, server_id);
            }

            http_header::set_remote_connection_info(
                &mut rsp.hop_by_hop_headers,
                server_id,
                self.tcp_notes.bind,
                self.tcp_notes.local,
                self.tcp_notes.next,
                &self.tcp_notes.expire,
            );

            if let Some(egress_info) = &self.tcp_notes.egress {
                http_header::set_dynamic_egress_info(
                    &mut rsp.hop_by_hop_headers,
                    server_id,
                    egress_info,
                );
            }
        }

        if self.ctx.server_config.echo_chained_info {
            if let Some(addr) = self.tcp_notes.chained.target_addr {
                http_header::set_upstream_addr(&mut rsp.hop_by_hop_headers, addr);
            }

            if let Some(addr) = self.tcp_notes.chained.outgoing_addr {
                http_header::set_outgoing_ip(&mut rsp.hop_by_hop_headers, addr);
            }
        }
    }

    async fn send_response_header<W>(
        &mut self,
        clt_w: &mut W,
        rsp: &HttpForwardRemoteResponse,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        let buf = rsp.serialize();
        clt_w
            .write_all(buf.as_ref())
            .await
            .map_err(ServerTaskError::ClientTcpWriteFailed)?;
        clt_w
            .flush()
            .await
            .map_err(ServerTaskError::ClientTcpWriteFailed)
    }
}
