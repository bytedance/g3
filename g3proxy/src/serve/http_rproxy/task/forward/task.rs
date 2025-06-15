/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use futures_util::FutureExt;
use http::header;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, AsyncWriteExt};

use g3_http::client::HttpForwardRemoteResponse;
use g3_http::server::HttpProxyClientRequest;
use g3_http::{HttpBodyReader, HttpBodyType};
use g3_io_ext::{
    GlobalLimitGroup, LimitedBufReadExt, LimitedReadExt, LimitedWriteExt, StreamCopy,
    StreamCopyError,
};
use g3_types::acl::AclAction;

use super::protocol::{HttpClientReader, HttpClientWriter, HttpRProxyRequest};
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
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::serve::http_rproxy::host::HttpHost;
use crate::serve::{
    ServerStats, ServerTaskError, ServerTaskForbiddenError, ServerTaskNotes, ServerTaskResult,
    ServerTaskStage,
};

pub(crate) struct HttpRProxyForwardTask<'a> {
    ctx: Arc<CommonTaskContext>,
    host: Arc<HttpHost>,
    req: &'a HttpProxyClientRequest,
    is_https: bool,
    should_close: bool,
    send_error_response: bool,
    task_notes: ServerTaskNotes,
    http_notes: HttpForwardTaskNotes,
    tcp_notes: TcpConnectTaskNotes,
    task_stats: Arc<HttpForwardTaskStats>,
    max_idle_count: usize,
    started: bool,
}

impl Drop for HttpRProxyForwardTask<'_> {
    fn drop(&mut self) {
        if self.started {
            self.post_stop();
            self.started = false;
        }
    }
}

impl<'a> HttpRProxyForwardTask<'a> {
    pub(crate) fn new(
        ctx: &Arc<CommonTaskContext>,
        req: &'a HttpRProxyRequest<impl AsyncRead>,
        host: Arc<HttpHost>,
        task_notes: ServerTaskNotes,
    ) -> Self {
        let uri_log_max_chars = task_notes
            .user_ctx()
            .and_then(|c| c.user_config().log_uri_max_chars)
            .unwrap_or(ctx.server_config.log_uri_max_chars);
        let http_notes = HttpForwardTaskNotes::new(
            req.time_received,
            task_notes.task_created_instant(),
            req.inner.method.clone(),
            req.inner.uri.clone(),
            uri_log_max_chars,
        );
        let is_https = host.tls_client.is_some();
        let max_idle_count = task_notes
            .user_ctx()
            .and_then(|c| c.user().task_max_idle_count())
            .unwrap_or(ctx.server_config.task_idle_max_count);
        HttpRProxyForwardTask {
            ctx: Arc::clone(ctx),
            host,
            req: &req.inner,
            is_https,
            should_close: !req.inner.keep_alive(),
            send_error_response: true,
            task_notes,
            http_notes,
            tcp_notes: TcpConnectTaskNotes::default(),
            task_stats: Arc::new(HttpForwardTaskStats::default()),
            max_idle_count,
            started: false,
        }
    }

    #[inline]
    pub(crate) fn should_close(&self) -> bool {
        self.should_close
    }

    fn enable_custom_header_for_local_reply(&self, _rsp: &mut HttpProxyClientResponse) {
        if let Some(_server_id) = &self.ctx.server_config.server_id {
            // TODO custom header
        }
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

    async fn reply_connect_err<W>(&mut self, e: &TcpConnectError, clt_w: &mut W)
    where
        W: AsyncWrite + Unpin,
    {
        let mut rsp = HttpProxyClientResponse::from_tcp_connect_error(
            e,
            self.req.version,
            self.should_close || self.req.body_type().is_some(),
        );

        self.enable_custom_header_for_local_reply(&mut rsp);

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
            self.enable_custom_header_for_local_reply(&mut rsp);

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

    fn get_log_context(&self) -> Option<TaskLogForHttpForward> {
        let Some(logger) = &self.ctx.task_logger else {
            return None;
        };

        let http_user_agent = self
            .req
            .end_to_end_headers
            .get(header::USER_AGENT)
            .map(|v| v.to_str());
        Some(TaskLogForHttpForward {
            logger,
            upstream: self.host.config.upstream(),
            task_notes: &self.task_notes,
            http_notes: &self.http_notes,
            http_user_agent,
            tcp_notes: &self.tcp_notes,
            client_rd_bytes: self.task_stats.clt.read.get_bytes(),
            client_wr_bytes: self.task_stats.clt.write.get_bytes(),
            remote_rd_bytes: self.task_stats.ups.read.get_bytes(),
            remote_wr_bytes: self.task_stats.ups.write.get_bytes(),
        })
    }

    pub(crate) async fn run<CDR, CDW>(
        &mut self,
        clt_r: &mut Option<HttpClientReader<CDR>>,
        clt_w: &mut HttpClientWriter<CDW>,
        fwd_ctx: &mut BoxHttpForwardContext,
    ) where
        CDR: AsyncRead + Unpin,
        CDW: AsyncWrite + Unpin,
    {
        self.pre_start();
        let e = match self.run_forward(clt_r, clt_w, fwd_ctx).await {
            Ok(()) => ServerTaskError::Finished,
            Err(e) => e,
        };
        if let Some(log_ctx) = self.get_log_context() {
            log_ctx.log(&e);
        }
    }

    fn pre_start(&mut self) {
        self.ctx.server_stats.task_http_forward.add_task();
        self.ctx.server_stats.task_http_forward.inc_alive_task();

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.foreach_req_stats(|s| {
                s.req_total.add_http_forward(self.is_https);
                s.req_alive.add_http_forward(self.is_https);
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
        self.ctx.server_stats.task_http_forward.dec_alive_task();

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.foreach_req_stats(|s| s.req_alive.del_http_forward(self.is_https));

            if let Some(user_req_alive_permit) = self.task_notes.user_req_alive_permit.take() {
                drop(user_req_alive_permit);
            }
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
                    self.ctx.server_stats.share_extra_tags(),
                );
                for s in &user_io_stats {
                    s.io.https_forward.add_in_bytes(origin_header_size);
                }
                wrapper_stats.push_user_io_stats(user_io_stats);

                let user_config = user_ctx.user_config();
                if user_config
                    .tcp_sock_speed_limit
                    .eq(&self.ctx.server_config.tcp_sock_speed_limit)
                {
                    None
                } else {
                    let limit_config = user_config
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
                    self.ctx.server_stats.share_extra_tags(),
                );
                for s in &user_io_stats {
                    s.io.http_forward.add_in_bytes(origin_header_size);
                }
                wrapper_stats.push_user_io_stats(user_io_stats);

                let user_config = user_ctx.user_config();
                if user_config
                    .tcp_sock_speed_limit
                    .eq(&self.ctx.server_config.tcp_sock_speed_limit)
                {
                    None
                } else {
                    let limit_config = user_config
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

        clt_w.retain_global_limiter_by_group(GlobalLimitGroup::Server);
        if let Some(br) = clt_r {
            br.reset_buffer_stats(clt_r_stats);
            clt_w.reset_stats(clt_w_stats);
            if let Some(limit_config) = &limit_config {
                br.reset_local_limit(limit_config.shift_millis, limit_config.max_north);
                clt_w.reset_local_limit(limit_config.shift_millis, limit_config.max_south);
            }
            if let Some(user_ctx) = self.task_notes.user_ctx() {
                let user = user_ctx.user();
                if let Some(limiter) = user.tcp_all_upload_speed_limit() {
                    limiter.try_consume(origin_header_size);
                    br.add_global_limiter(limiter.clone());
                }
                if let Some(limiter) = user.tcp_all_download_speed_limit() {
                    clt_w.add_global_limiter(limiter.clone());
                }
            }
        } else {
            clt_w.reset_stats(clt_w_stats);
            if let Some(limit_config) = &limit_config {
                clt_w.reset_local_limit(limit_config.shift_millis, limit_config.max_south);
            }
            if let Some(user_ctx) = self.task_notes.user_ctx() {
                let user = user_ctx.user();
                if let Some(limiter) = user.tcp_all_upload_speed_limit() {
                    limiter.try_consume(origin_header_size);
                }
                if let Some(limiter) = user.tcp_all_download_speed_limit() {
                    clt_w.add_global_limiter(limiter.clone());
                }
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
        CDR: AsyncRead + Unpin,
        CDW: AsyncWrite + Unpin,
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

            let action = user_ctx.check_upstream(self.host.config.upstream());
            self.handle_user_upstream_acl_action(action, clt_w).await?;

            if let Some(action) = user_ctx.check_http_user_agent(&self.req.end_to_end_headers) {
                self.handle_user_ua_acl_action(action, clt_w).await?;
            }

            upstream_keepalive =
                upstream_keepalive.adjust_to(user_ctx.user_config().http_upstream_keepalive);
            tcp_client_misc_opts = user_ctx
                .user_config()
                .tcp_client_misc_opts(&tcp_client_misc_opts);
        }

        // set client side socket options
        self.ctx
            .cc_info
            .tcp_sock_set_raw_opts(&tcp_client_misc_opts, true)
            .map_err(|_| {
                ServerTaskError::InternalServerError("failed to set client socket options")
            })?;

        self.setup_clt_limit_and_stats(clt_r, clt_w);

        fwd_ctx.prepare_connection(self.host.config.upstream(), self.is_https);

        if let Some(mut connection) = fwd_ctx
            .get_alive_connection(
                &self.task_notes,
                self.task_stats.clone(),
                upstream_keepalive.idle_expire(),
            )
            .await
        {
            self.task_notes.stage = ServerTaskStage::Connected;
            self.http_notes.reused_connection = true;
            fwd_ctx.fetch_tcp_notes(&mut self.tcp_notes);
            self.http_notes.retry_new_connection = false;
            if let Some(user_ctx) = self.task_notes.user_ctx() {
                user_ctx.foreach_req_stats(|s| s.req_reuse.add_http_forward(self.is_https));
            }

            if self.ctx.server_config.flush_task_log_on_connected {
                if let Some(log_ctx) = self.get_log_context() {
                    log_ctx.log_connected();
                }
            }

            connection
                .0
                .prepare_new(&self.task_notes, self.host.config.upstream());
            self.mark_relaying();

            let r = self
                .run_with_connection(fwd_ctx, clt_r, clt_w, connection)
                .await;
            match r {
                Ok(ups_s) => {
                    self.save_or_close(fwd_ctx, clt_w, ups_s).await;
                    return Ok(());
                }
                Err(e) => {
                    if self.http_notes.retry_new_connection {
                        if let Some(log_ctx) = self.get_log_context() {
                            log_ctx.log(&e);
                        }
                        self.task_stats.ups.reset();
                        // continue to make new connection
                        if let Some(user_ctx) = self.task_notes.user_ctx() {
                            user_ctx
                                .foreach_req_stats(|s| s.req_renew.add_http_forward(self.is_https));
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

        let connection = self.get_new_connection(fwd_ctx, clt_w).await?;
        match self
            .run_with_connection(fwd_ctx, clt_r, clt_w, connection)
            .await
        {
            Ok(ups_s) => {
                self.save_or_close(fwd_ctx, clt_w, ups_s).await;
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

    async fn save_or_close<CDW>(
        &self,
        fwd_ctx: &mut BoxHttpForwardContext,
        clt_w: &mut HttpClientWriter<CDW>,
        ups_s: Option<BoxHttpForwardConnection>,
    ) where
        CDW: AsyncWrite + Unpin,
    {
        if self.should_close {
            if let Some(mut connection) = ups_s {
                let _ = connection.0.shutdown().await;
            }
            let _ = clt_w.shutdown().await;
        } else if let Some(connection) = ups_s {
            fwd_ctx.save_alive_connection(connection);
        }
    }

    async fn get_new_connection<CDW>(
        &mut self,
        fwd_ctx: &mut BoxHttpForwardContext,
        clt_w: &mut HttpClientWriter<CDW>,
    ) -> ServerTaskResult<BoxHttpForwardConnection>
    where
        CDW: AsyncWrite + Unpin,
    {
        self.task_notes.stage = ServerTaskStage::Connecting;
        self.http_notes.reused_connection = false;

        match self.make_new_connection(fwd_ctx).await {
            Ok(mut connection) => {
                self.task_notes.stage = ServerTaskStage::Connected;
                fwd_ctx.fetch_tcp_notes(&mut self.tcp_notes);

                if self.ctx.server_config.flush_task_log_on_connected {
                    if let Some(log_ctx) = self.get_log_context() {
                        log_ctx.log_connected();
                    }
                }

                connection
                    .0
                    .prepare_new(&self.task_notes, self.host.config.upstream());
                self.mark_relaying();
                Ok(connection)
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
        if let Some(tls_client) = &self.host.tls_client {
            let task_conf = TlsConnectTaskConf {
                tcp: TcpConnectTaskConf {
                    upstream: self.host.config.upstream(),
                },
                tls_config: tls_client,
                tls_name: &self.host.config.tls_name,
            };
            fwd_ctx
                .make_new_https_connection(&task_conf, &self.task_notes, self.task_stats.clone())
                .await
        } else {
            let task_conf = TcpConnectTaskConf {
                upstream: self.host.config.upstream(),
            };
            fwd_ctx
                .make_new_http_connection(&task_conf, &self.task_notes, self.task_stats.clone())
                .await
        }
    }

    fn mark_relaying(&mut self) {
        self.task_notes.mark_relaying();
        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.foreach_req_stats(|s| s.req_ready.add_http_forward(self.is_https));
        }
    }

    async fn run_with_connection<CDR, CDW>(
        &mut self,
        fwd_ctx: &mut BoxHttpForwardContext,
        clt_r: &mut Option<HttpClientReader<CDR>>,
        clt_w: &mut HttpClientWriter<CDW>,
        mut ups_c: BoxHttpForwardConnection,
    ) -> ServerTaskResult<Option<BoxHttpForwardConnection>>
    where
        CDR: AsyncRead + Unpin,
        CDW: AsyncWrite + Unpin,
    {
        if self.http_notes.reused_connection {
            if let Some(r) = ups_c.1.fill_wait_data().now_or_never() {
                self.http_notes.retry_new_connection = true;
                return match r {
                    Ok(true) => Err(ServerTaskError::UpstreamAppError(anyhow!(
                        "unexpected data found when polling IDLE connection"
                    ))),
                    Ok(false) => Err(ServerTaskError::ClosedByUpstream),
                    Err(e) => Err(ServerTaskError::UpstreamReadFailed(e)),
                };
            }
        }

        match self.req.body_type() {
            Some(body_type) => {
                let Some(clt_r) = clt_r else {
                    return Err(ServerTaskError::InternalServerError(
                        "http body is expected but no body reader supplied",
                    ));
                };

                let mut clt_body_reader =
                    HttpBodyReader::new(clt_r, body_type, self.ctx.server_config.body_line_max_len);

                if self.req.end_to_end_headers.contains_key(header::EXPECT) {
                    return self
                        .run_with_body(None, &mut clt_body_reader, clt_w, ups_c)
                        .await;
                }

                let mut fast_read_buf = vec![0u8; self.ctx.server_config.tcp_copy.buffer_size()];
                let nr = clt_body_reader
                    .read_all_now(&mut fast_read_buf)
                    .await
                    .map_err(ServerTaskError::ClientTcpReadFailed)?
                    .ok_or(ServerTaskError::ClosedByClient)?;
                if nr == 0 {
                    return self
                        .run_with_body(None, &mut clt_body_reader, clt_w, ups_c)
                        .await;
                }
                fast_read_buf.truncate(nr);

                if clt_body_reader.finished() {
                    return self
                        .run_with_all_body(fwd_ctx, fast_read_buf, clt_w, ups_c)
                        .await;
                }

                loop {
                    match self
                        .run_with_body(
                            Some(fast_read_buf.clone()),
                            &mut clt_body_reader,
                            clt_w,
                            ups_c,
                        )
                        .await
                    {
                        Ok(r) => return Ok(r),
                        Err(e) => {
                            if self.http_notes.reused_connection
                                && self.http_notes.retry_new_connection
                            {
                                if let Some(log_ctx) = self.get_log_context() {
                                    log_ctx.log(&e);
                                }
                                self.task_stats.ups.reset();
                                ups_c = self.get_new_connection(fwd_ctx, clt_w).await?;
                            } else {
                                self.http_notes.retry_new_connection = false;
                                return Err(e);
                            }
                        }
                    }
                }
            }
            None => self.run_without_body(clt_w, ups_c).await,
        }
    }

    async fn run_without_body<W>(
        &mut self,
        clt_w: &mut W,
        mut ups_c: BoxHttpForwardConnection,
    ) -> ServerTaskResult<Option<BoxHttpForwardConnection>>
    where
        W: AsyncWrite + Unpin,
    {
        let ups_w = &mut ups_c.0;
        let ups_r = &mut ups_c.1;

        self.http_notes.retry_new_connection = true;
        ups_w
            .send_request_header(self.req, None)
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)?;
        ups_w
            .flush()
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)?;
        self.http_notes.mark_req_send_hdr();
        self.http_notes.mark_req_no_body();

        let mut rsp_header = match tokio::time::timeout(
            self.ctx.server_config.timeout.recv_rsp_header,
            self.recv_response_header(ups_r),
        )
        .await
        {
            Ok(Ok(rsp_header)) => {
                self.http_notes.retry_new_connection = false;
                rsp_header
            }
            Ok(Err(e)) => {
                if self.task_stats.ups.read.get_bytes() == 0 {
                    self.http_notes.retry_new_connection = matches!(
                        e,
                        ServerTaskError::ClosedByUpstream | ServerTaskError::UpstreamReadFailed(_)
                    );
                } else {
                    self.http_notes.retry_new_connection = false;
                }
                return Err(e);
            }
            Err(_) => {
                self.http_notes.retry_new_connection = false;
                return Err(ServerTaskError::UpstreamAppTimeout(
                    "timeout to receive response header",
                ));
            }
        };
        self.http_notes.mark_rsp_recv_hdr();

        self.update_response_header(&mut rsp_header);
        self.send_response(clt_w, ups_r, &rsp_header).await?;

        self.task_notes.stage = ServerTaskStage::Finished;
        Ok(Some(ups_c))
    }

    async fn send_full_req_and_recv_rsp(
        &mut self,
        body: &[u8],
        ups_r: &mut BoxHttpForwardReader,
        ups_w: &mut BoxHttpForwardWriter,
    ) -> ServerTaskResult<HttpForwardRemoteResponse> {
        self.http_notes.retry_new_connection = true;

        ups_w
            .send_request_header(self.req, Some(body))
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)?;
        ups_w
            .flush()
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)?;
        self.http_notes.mark_req_send_hdr();
        self.http_notes.mark_req_send_all();

        match tokio::time::timeout(
            self.ctx.server_config.timeout.recv_rsp_header,
            self.recv_response_header(ups_r),
        )
        .await
        {
            Ok(Ok(rsp_header)) => {
                self.http_notes.retry_new_connection = false;
                Ok(rsp_header)
            }
            Ok(Err(e)) => {
                if self.task_stats.ups.read.get_bytes() == 0 {
                    self.http_notes.retry_new_connection = matches!(
                        e,
                        ServerTaskError::ClosedByUpstream | ServerTaskError::UpstreamReadFailed(_)
                    );
                } else {
                    self.http_notes.retry_new_connection = false;
                }
                Err(e)
            }
            Err(_) => {
                self.http_notes.retry_new_connection = false;
                Err(ServerTaskError::UpstreamAppTimeout(
                    "timeout to receive response header",
                ))
            }
        }
    }

    async fn run_with_all_body<CDW>(
        &mut self,
        fwd_ctx: &mut BoxHttpForwardContext,
        body: Vec<u8>,
        clt_w: &mut HttpClientWriter<CDW>,
        mut ups_c: BoxHttpForwardConnection,
    ) -> ServerTaskResult<Option<BoxHttpForwardConnection>>
    where
        CDW: AsyncWrite + Unpin,
    {
        loop {
            let ups_w = &mut ups_c.0;
            let ups_r = &mut ups_c.1;

            let mut rsp_header = match self
                .send_full_req_and_recv_rsp(body.as_slice(), ups_r, ups_w)
                .await
            {
                Ok(rsp_header) => rsp_header,
                Err(e) => {
                    if self.http_notes.reused_connection && self.http_notes.retry_new_connection {
                        if let Some(log_ctx) = self.get_log_context() {
                            log_ctx.log(&e);
                        }
                        self.task_stats.ups.reset();
                        ups_c = self.get_new_connection(fwd_ctx, clt_w).await?;
                        continue;
                    } else {
                        self.http_notes.retry_new_connection = false;
                        return Err(e);
                    }
                }
            };

            self.http_notes.mark_rsp_recv_hdr();
            self.update_response_header(&mut rsp_header);

            self.send_response(clt_w, ups_r, &rsp_header).await?;

            self.task_notes.stage = ServerTaskStage::Finished;
            return Ok(Some(ups_c));
        }
    }

    async fn run_with_body<R, CDW>(
        &mut self,
        fast_read_buf: Option<Vec<u8>>,
        clt_body_reader: &mut HttpBodyReader<'_, R>,
        clt_w: &mut HttpClientWriter<CDW>,
        mut ups_c: BoxHttpForwardConnection,
    ) -> ServerTaskResult<Option<BoxHttpForwardConnection>>
    where
        R: AsyncBufRead + Unpin,
        CDW: AsyncWrite + Unpin,
    {
        let ups_w = &mut ups_c.0;
        let ups_r = &mut ups_c.1;

        self.http_notes.retry_new_connection = true;
        ups_w
            .send_request_header(self.req, None)
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)?;
        ups_w
            .flush()
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)?;
        self.http_notes.mark_req_send_hdr();
        self.http_notes.retry_new_connection = false;

        let mut clt_to_ups = match fast_read_buf {
            Some(buf) => StreamCopy::with_data(
                clt_body_reader,
                ups_w,
                &self.ctx.server_config.tcp_copy,
                buf,
            ),
            None => StreamCopy::new(clt_body_reader, ups_w, &self.ctx.server_config.tcp_copy),
        };

        let mut rsp_header: Option<HttpForwardRemoteResponse> = None;

        let mut idle_interval = self.ctx.idle_wheel.register();
        let mut log_interval = self.ctx.get_log_interval();
        let mut idle_count = 0;
        loop {
            tokio::select! {
                biased;

                r = ups_r.fill_wait_data() => {
                    match r {
                        Ok(true) => {
                            // we got some data from upstream
                            let hdr = self.recv_response_header(ups_r).await?;
                            match hdr.code {
                                100 | 103 => {
                                    // CONTINUE | Early Hints
                                    self.send_response_header(clt_w, &hdr).await?;
                                }
                                _ => {
                                    rsp_header = Some(hdr);
                                    break;
                                }
                            }
                        }
                        Ok(false) => {
                             if clt_to_ups.read_size() == 0 {
                                self.http_notes.retry_new_connection = true;
                            }
                            return Err(ServerTaskError::ClosedByUpstream);
                        },
                        Err(e) => {
                            if clt_to_ups.read_size() == 0 {
                                self.http_notes.retry_new_connection = true;
                            }
                            return Err(ServerTaskError::UpstreamReadFailed(e));
                        },
                    }
                }
                r = &mut clt_to_ups => {
                    r.map_err(|e| match e {
                        StreamCopyError::ReadFailed(e) => ServerTaskError::ClientTcpReadFailed(e),
                        StreamCopyError::WriteFailed(e) => ServerTaskError::UpstreamWriteFailed(e),
                    })?;
                    self.http_notes.mark_req_send_all();
                    break;
                }
                _ = log_interval.tick() => {
                    if let Some(log_ctx) = self.get_log_context() {
                        log_ctx.log_periodic();
                    }
                }
                n = idle_interval.tick() => {
                    if clt_to_ups.is_idle() {
                        idle_count += n;

                        if let Some(user_ctx) = self.task_notes.user_ctx() {
                            let user = user_ctx.user();
                            if user.is_blocked() {
                                return Err(ServerTaskError::CanceledAsUserBlocked);
                            }
                        }

                        if idle_count >= self.max_idle_count {
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
                        ));
                    }
                }
            }
        };
        self.http_notes.mark_rsp_recv_hdr();

        self.update_response_header(&mut rsp_header);
        self.send_response(clt_w, ups_r, &rsp_header).await?;

        self.task_notes.stage = ServerTaskStage::Finished;
        if close_remote {
            let _ = ups_w.shutdown().await;
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
        loop {
            let hdr = self.recv_response_header(ups_r).await?;
            match hdr.code {
                100 => {
                    // HTTP CONTINUE
                    self.send_response_header(clt_w, &hdr).await?;
                    // recv the final response header
                    return self.recv_response_header(ups_r).await;
                }
                103 => {
                    // HTTP Early Hints
                    self.send_response_header(clt_w, &hdr).await?;
                }
                _ => {
                    return Ok(hdr);
                }
            }
        }
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
        rsp_header: &HttpForwardRemoteResponse,
    ) -> ServerTaskResult<()>
    where
        R: AsyncBufRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        if !rsp_header.keep_alive() {
            self.should_close = true;
        }
        self.send_error_response = false;
        self.http_notes.origin_status = rsp_header.code;

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

        let mut ups_to_clt = StreamCopy::with_data(
            &mut body_reader,
            clt_w,
            &self.ctx.server_config.tcp_copy,
            header,
        );

        let mut idle_interval = self.ctx.idle_wheel.register();
        let mut log_interval = self.ctx.get_log_interval();
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
                        Err(StreamCopyError::ReadFailed(e)) => {
                            if ups_to_clt.copied_size() < header_len {
                                let _ = ups_to_clt.write_flush().await; // flush rsp header to client
                            }
                            Err(ServerTaskError::UpstreamReadFailed(e))
                        }
                        Err(StreamCopyError::WriteFailed(e)) => Err(ServerTaskError::ClientTcpWriteFailed(e)),
                    };
                }
                _ = log_interval.tick() => {
                    if let Some(log_ctx) = self.get_log_context() {
                       log_ctx.log_periodic();
                    }
                }
                n = idle_interval.tick() => {
                    if ups_to_clt.is_idle() {
                        idle_count += n;

                        if let Some(user_ctx) = self.task_notes.user_ctx() {
                            let user = user_ctx.user();
                            if user.is_blocked() {
                                if ups_to_clt.copied_size() < header_len {
                                    let _ = ups_to_clt.write_flush().await; // flush rsp header to client
                                }
                                return Err(ServerTaskError::CanceledAsUserBlocked);
                            }
                        }

                        if idle_count >= self.max_idle_count {
                            return if ups_to_clt.no_cached_data() {
                                Err(ServerTaskError::UpstreamAppTimeout("idle while reading response body"))
                            } else {
                                Err(ServerTaskError::ClientAppTimeout("idle while sending response body"))
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
        if self.should_close {
            rsp.set_no_keep_alive();
        }

        if let Some(_server_id) = &self.ctx.server_config.server_id {
            // TODO custom header
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
            .write_all_flush(buf.as_ref())
            .await
            .map_err(ServerTaskError::ClientTcpWriteFailed)
    }
}
