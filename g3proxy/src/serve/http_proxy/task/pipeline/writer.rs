/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use ahash::AHashMap;
use arcstr::ArcStr;
use log::debug;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

use g3_io_ext::{ArcLimitedWriterStats, LimitedWriter};
use g3_types::auth::UserAuthError;
use g3_types::net::{HttpAuth, HttpProxySubProtocol};

use super::protocol::{HttpClientReader, HttpClientWriter, HttpProxyRequest};
use super::{
    CommonTaskContext, FtpOverHttpTask, HttpProxyCltWrapperStats, HttpProxyConnectTask,
    HttpProxyForwardTask, HttpProxyPipelineStats, HttpProxyUntrustedTask,
};
use crate::audit::AuditContext;
use crate::auth::{UserContext, UserGroup, UserRequestStats};
use crate::config::server::ServerConfig;
use crate::escape::EgressPathSelection;
use crate::module::http_forward::{BoxHttpForwardContext, HttpProxyClientResponse};
use crate::serve::{ServerStats, ServerTaskNotes};

struct UserData {
    req_stats: Arc<UserRequestStats>,
    site_req_stats: Option<Arc<UserRequestStats>>,
    count: usize,
}

impl Drop for UserData {
    fn drop(&mut self) {
        self.req_stats.l7_conn_alive.dec_http();
        if let Some(site_req_stats) = &self.site_req_stats {
            site_req_stats.l7_conn_alive.dec_http();
        }
    }
}

struct RequestCount {
    passed_users: AHashMap<ArcStr, UserData>,
    anonymous: usize,
    auth_failed: usize,
    invalid: usize,
    consequent_auth_failed: usize,
}

impl Default for RequestCount {
    fn default() -> Self {
        RequestCount {
            passed_users: AHashMap::new(),
            anonymous: 0,
            auth_failed: 0,
            invalid: 0,
            consequent_auth_failed: 0,
        }
    }
}

pub(crate) struct HttpProxyPipelineWriterTask<CDR, CDW> {
    ctx: Arc<CommonTaskContext>,
    audit_ctx: AuditContext,
    user_group: Option<Arc<UserGroup>>,
    task_queue: mpsc::Receiver<Result<HttpProxyRequest<CDR>, HttpProxyClientResponse>>,
    stream_writer: Option<HttpClientWriter<CDW>>,
    forward_context: BoxHttpForwardContext,
    wrapper_stats: ArcLimitedWriterStats,
    pipeline_stats: Arc<HttpProxyPipelineStats>,
    req_count: RequestCount,
}

enum LoopAction {
    Continue,
    Break,
}

impl<CDR, CDW> HttpProxyPipelineWriterTask<CDR, CDW>
where
    CDR: AsyncRead + Send + Sync + Unpin + 'static,
    CDW: AsyncWrite + Send + Sync + Unpin + 'static,
{
    pub(crate) fn new(
        ctx: &Arc<CommonTaskContext>,
        audit_ctx: AuditContext,
        user_group: Option<Arc<UserGroup>>,
        task_receiver: mpsc::Receiver<Result<HttpProxyRequest<CDR>, HttpProxyClientResponse>>,
        write_half: CDW,
        pipeline_stats: &Arc<HttpProxyPipelineStats>,
    ) -> Self {
        let forward_context = ctx
            .escaper
            .new_http_forward_context(Arc::clone(&ctx.escaper));
        let clt_w_stats = HttpProxyCltWrapperStats::new_for_writer(&ctx.server_stats);
        let limit_config = &ctx.server_config.tcp_sock_speed_limit;
        let clt_w = LimitedWriter::local_limited(
            write_half,
            limit_config.shift_millis,
            limit_config.max_south,
            Arc::clone(&clt_w_stats),
        );
        HttpProxyPipelineWriterTask {
            ctx: Arc::clone(ctx),
            audit_ctx,
            user_group,
            task_queue: task_receiver,
            stream_writer: Some(clt_w),
            forward_context,
            wrapper_stats: clt_w_stats,
            pipeline_stats: Arc::clone(pipeline_stats),
            req_count: RequestCount::default(),
        }
    }

    fn do_auth(
        &mut self,
        req: &HttpProxyRequest<CDR>,
    ) -> Result<Option<UserContext>, UserAuthError> {
        if let Some(user_group) = &self.user_group {
            let mut user_ctx = match &req.inner.auth_info {
                HttpAuth::None => user_group
                    .get_anonymous_user()
                    .map(|(user, user_type)| {
                        UserContext::new(
                            None,
                            user,
                            user_type,
                            self.ctx.server_config.name(),
                            self.ctx.server_stats.share_extra_tags(),
                        )
                    })
                    .ok_or(UserAuthError::NoUserSupplied)?,
                HttpAuth::Basic(v) => {
                    let username = v.username.as_original();
                    let username = self
                        .ctx
                        .server_config
                        .username_params
                        .as_ref()
                        .map(|c| c.real_username(username))
                        .unwrap_or(username);
                    user_group.check_user_with_password(
                        username,
                        &v.password,
                        self.ctx.server_config.name(),
                        self.ctx.server_stats.share_extra_tags(),
                    )?
                }
            };
            user_ctx.check_client_addr(self.ctx.client_addr())?;

            user_ctx.check_in_site(
                self.ctx.server_config.name(),
                self.ctx.server_stats.share_extra_tags(),
                &req.upstream,
            );
            self.req_count
                .passed_users
                .entry(user_ctx.user_name().clone())
                .and_modify(|e| {
                    user_ctx.mark_reused_client_connection();
                    e.count += 1;
                })
                .or_insert_with(|| {
                    let req_stats = user_ctx.req_stats().clone();
                    req_stats.conn_total.add_http();
                    req_stats.l7_conn_alive.inc_http();
                    let site_req_stats = if let Some(site_req_stats) = user_ctx.site_req_stats() {
                        site_req_stats.conn_total.add_http();
                        site_req_stats.l7_conn_alive.inc_http();
                        Some(Arc::clone(site_req_stats))
                    } else {
                        None
                    };
                    UserData {
                        req_stats,
                        site_req_stats,
                        count: 1,
                    }
                });
            Ok(Some(user_ctx))
        } else {
            self.req_count.anonymous += 1;
            Ok(None)
        }
    }

    pub(crate) async fn into_running(mut self) {
        loop {
            let res = match self.task_queue.recv().await {
                Some(Ok(req)) => {
                    let res = match self.do_auth(&req) {
                        Ok(user_ctx) => {
                            self.req_count.consequent_auth_failed = 0;
                            self.run(req, user_ctx).await
                        }
                        Err(e) => {
                            self.req_count.consequent_auth_failed += 1;
                            self.req_count.auth_failed += 1;
                            self.run_untrusted(req, e.blocked_delay()).await
                        }
                    };
                    self.pipeline_stats.del_task();
                    res
                }
                Some(Err(rsp)) => {
                    // the response will always be `Connection: Close`
                    self.req_count.invalid += 1;
                    if !self.ctx.server_config.no_early_error_reply
                        && let Some(stream_w) = &mut self.stream_writer
                    {
                        let _ = rsp.reply_err_to_request(stream_w).await;
                    }

                    self.notify_reader_to_close();
                    LoopAction::Break
                }
                None => LoopAction::Break,
            };
            match res {
                LoopAction::Continue => {}
                LoopAction::Break => {
                    break;
                }
            }
        }
    }

    fn get_egress_path_selection(
        &self,
        req: &mut HttpProxyRequest<CDR>,
    ) -> Result<Option<EgressPathSelection>, ()> {
        let mut egress_path = EgressPathSelection::default();

        if let Some(header) = &self.ctx.server_config.egress_path_selection_header {
            // check and remove the custom header
            if let Some(value) = req.inner.end_to_end_headers.remove(header) {
                match usize::from_str(value.to_str()) {
                    Ok(id) => egress_path.set_number_id(self.ctx.server_config.name().clone(), id),
                    Err(e) => {
                        debug!("invalid egress path number id value in header {header}: {e}");
                        return Err(());
                    }
                }
            }
        }

        // Optional: compute username-param-derived escaper address and store override
        if let Some(name_params) = &self.ctx.server_config.username_params
            && let HttpAuth::Basic(v) = &req.inner.auth_info
        {
            match name_params.parse_egress_upstream_http(v.username.as_original()) {
                Ok(Some(ups)) => {
                    debug!(
                        "[{}] http username params -> next proxy {}",
                        self.ctx.server_config.name(),
                        ups.addr
                    );
                    egress_path.set_upstream(self.ctx.escaper.name().clone(), ups);
                }
                Ok(None) => {}
                Err(e) => {
                    debug!("failed to get upstream addr from username: {e}");
                    return Err(());
                }
            }
        }

        if egress_path.is_empty() {
            Ok(None)
        } else {
            Ok(Some(egress_path))
        }
    }

    async fn run(
        &mut self,
        mut req: HttpProxyRequest<CDR>,
        user_ctx: Option<UserContext>,
    ) -> LoopAction {
        let Ok(path_selection) = self.get_egress_path_selection(&mut req) else {
            self.req_count.invalid += 1;
            // Bad request: unsupported param combo or invalid params
            if let Some(stream_w) = &mut self.stream_writer {
                let rsp = HttpProxyClientResponse::bad_request(req.inner.version);
                let _ = rsp.reply_err_to_request(stream_w).await;
            }
            self.notify_reader_to_close();
            return LoopAction::Break;
        };

        let task_notes = ServerTaskNotes::with_path_selection(
            self.ctx.cc_info.clone(),
            user_ctx,
            req.time_accepted.elapsed(),
            path_selection,
        );

        let mut audit_ctx = self.audit_ctx.clone();
        let remote_protocol = match req.client_protocol {
            HttpProxySubProtocol::TcpConnect => HttpProxySubProtocol::TcpConnect,
            HttpProxySubProtocol::HttpForward => {
                let _ = self
                    .forward_context
                    .check_in_final_escaper(&task_notes, &req.upstream, &mut audit_ctx)
                    .await;
                HttpProxySubProtocol::HttpForward
            }
            HttpProxySubProtocol::HttpsForward => {
                let forward_capability = self
                    .forward_context
                    .check_in_final_escaper(&task_notes, &req.upstream, &mut audit_ctx)
                    .await;
                if forward_capability.forward_https() {
                    HttpProxySubProtocol::HttpForward
                } else {
                    HttpProxySubProtocol::HttpsForward
                }
            }
            HttpProxySubProtocol::FtpOverHttp => {
                let forward_capability = self
                    .forward_context
                    .check_in_final_escaper(&task_notes, &req.upstream, &mut audit_ctx)
                    .await;
                if forward_capability.forward_ftp(&req.inner.method) {
                    HttpProxySubProtocol::HttpForward
                } else {
                    HttpProxySubProtocol::FtpOverHttp
                }
            }
        };

        match remote_protocol {
            HttpProxySubProtocol::TcpConnect => {
                if let (Some(mut stream_w), Some(stream_r)) =
                    (self.stream_writer.take(), req.body_reader.take())
                {
                    let mut connect_task =
                        HttpProxyConnectTask::new(&self.ctx, audit_ctx, &req, task_notes);
                    connect_task.connect_to_upstream(&mut stream_w).await;
                    if connect_task.back_to_http() {
                        // reopen write end
                        self.stream_writer = Some(stream_w);
                        // reopen read end
                        if req.stream_sender.try_send(Some(stream_r)).is_err() {
                            // read end has closed, impossible as reader should be waiting this channel
                            LoopAction::Break
                        } else {
                            LoopAction::Continue
                        }
                    } else {
                        // close read end
                        let _ = req.stream_sender.try_send(None);
                        connect_task.into_running(stream_r.into_inner(), stream_w);
                        LoopAction::Break
                    }
                } else {
                    unreachable!()
                }
            }
            HttpProxySubProtocol::HttpForward | HttpProxySubProtocol::HttpsForward => {
                if let Some(mut stream_w) = self.stream_writer.take() {
                    match self
                        .run_forward(&mut stream_w, req, task_notes, audit_ctx, remote_protocol)
                        .await
                    {
                        LoopAction::Continue => {
                            self.reset_client_writer(stream_w);
                            LoopAction::Continue
                        }
                        LoopAction::Break => LoopAction::Break,
                    }
                } else {
                    unreachable!()
                }
            }
            HttpProxySubProtocol::FtpOverHttp => {
                if let (Some(mut stream_w), Some(stream_r)) =
                    (self.stream_writer.take(), req.body_reader.take())
                {
                    match self
                        .run_ftp_over_http(&mut stream_w, stream_r, req, task_notes)
                        .await
                    {
                        LoopAction::Continue => {
                            self.reset_client_writer(stream_w);
                            LoopAction::Continue
                        }
                        LoopAction::Break => LoopAction::Break,
                    }
                } else {
                    unreachable!()
                }
            }
        }
    }

    fn reset_client_writer(&mut self, mut stream_w: HttpClientWriter<CDW>) {
        stream_w.reset_stats(Arc::clone(&self.wrapper_stats));
        let limit_config = &self.ctx.server_config.tcp_sock_speed_limit;
        stream_w.reset_local_limit(limit_config.shift_millis, limit_config.max_south);
        self.stream_writer = Some(stream_w);
    }

    async fn run_untrusted(
        &mut self,
        mut req: HttpProxyRequest<CDR>,
        blocked_delay: Option<Duration>,
    ) -> LoopAction {
        if self.ctx.server_config.no_early_error_reply {
            if let Some(duration) = blocked_delay {
                self.ctx.server_stats.forbidden.add_user_blocked();

                // delay some time before close
                tokio::time::sleep(duration).await;
            } else {
                self.ctx.server_stats.forbidden.add_auth_failed();
            }

            self.notify_reader_to_close();
            LoopAction::Break
        } else if let Some(duration) = blocked_delay {
            self.ctx.server_stats.forbidden.add_user_blocked();

            // delay some time before reply
            tokio::time::sleep(duration).await;

            // user is blocked, always close the connection
            if let Some(clt_w) = &mut self.stream_writer {
                let rsp = HttpProxyClientResponse::forbidden(req.inner.version);
                // no custom header is set
                let _ = rsp.reply_err_to_request(clt_w).await;
            }

            self.notify_reader_to_close();
            LoopAction::Break
        } else if self.req_count.consequent_auth_failed > 1 {
            // if the previous request has already failed, close the connection
            self.ctx.server_stats.forbidden.add_auth_failed();

            if let Some(clt_w) = &mut self.stream_writer {
                // no custom header is set
                let _ = HttpProxyClientResponse::reply_proxy_auth_err(
                    req.inner.version,
                    clt_w,
                    &self.ctx.server_config.auth_realm,
                    true,
                )
                .await;
            }

            self.notify_reader_to_close();
            LoopAction::Break
        } else if let Some(clt_w) = &mut self.stream_writer {
            self.ctx.server_stats.forbidden.add_auth_failed();

            match req.body_reader.take() {
                Some(stream_r) => {
                    let mut untrusted_task = HttpProxyUntrustedTask::new(&self.ctx, &req);
                    let mut clt_r = Some(stream_r);
                    untrusted_task.run(&mut clt_r, clt_w).await;
                    if untrusted_task.should_close() {
                        // close read end
                        let _ = req.stream_sender.try_send(None);
                        LoopAction::Break
                    } else {
                        // reopen read end
                        if req.stream_sender.try_send(clt_r).is_err() {
                            // read end has closed, impossible as reader should be waiting this channel
                            LoopAction::Break
                        } else {
                            LoopAction::Continue
                        }
                    }
                }
                None => {
                    let mut untrusted_task = HttpProxyUntrustedTask::new(&self.ctx, &req);
                    let mut clt_r = None;
                    untrusted_task.run::<CDR, CDW>(&mut clt_r, clt_w).await;
                    if untrusted_task.should_close() {
                        // i.e. ups_s io error may cause response data to be corrupted
                        self.notify_reader_to_close();
                        LoopAction::Break
                    } else {
                        LoopAction::Continue
                    }
                }
            }
        } else {
            self.ctx.server_stats.forbidden.add_auth_failed();

            // should be impossible
            self.notify_reader_to_close();
            LoopAction::Break
        }
    }

    async fn run_forward(
        &mut self,
        clt_w: &mut HttpClientWriter<CDW>,
        mut req: HttpProxyRequest<CDR>,
        task_notes: ServerTaskNotes,
        audit_ctx: AuditContext,
        remote_protocol: HttpProxySubProtocol,
    ) -> LoopAction {
        let is_https = match remote_protocol {
            HttpProxySubProtocol::HttpForward => {
                if self.ctx.server_config.drop_default_port_in_host && req.upstream.port() == 80 {
                    req.drop_default_port_in_host();
                }
                false
            }
            HttpProxySubProtocol::HttpsForward => {
                if self.ctx.server_config.drop_default_port_in_host && req.upstream.port() == 443 {
                    req.drop_default_port_in_host();
                }
                true
            }
            _ => unreachable!(),
        };

        match req.body_reader.take() {
            Some(stream_r) => {
                // we have a body, or we need to close the connection
                // we may need to send stream_r back if we have a body
                let mut forward_task =
                    HttpProxyForwardTask::new(&self.ctx, audit_ctx, &req, is_https, task_notes);
                let mut clt_r = Some(stream_r);
                forward_task
                    .run(&mut clt_r, clt_w, &mut self.forward_context)
                    .await;
                if forward_task.should_close() {
                    // close read end
                    let _ = req.stream_sender.try_send(None);
                    LoopAction::Break
                } else {
                    // reopen read end
                    if req.stream_sender.try_send(clt_r).is_err() {
                        // read end has closed, impossible as reader should be waiting this channel
                        LoopAction::Break
                    } else {
                        LoopAction::Continue
                    }
                }
            }
            None => {
                // no http body, and the connection is expected to keep alive from the client side
                let mut forward_task =
                    HttpProxyForwardTask::new(&self.ctx, audit_ctx, &req, is_https, task_notes);
                let mut clt_r = None;
                forward_task
                    .run::<CDR, CDW>(&mut clt_r, clt_w, &mut self.forward_context)
                    .await;
                if forward_task.should_close() {
                    // i.e. ups_s io error may cause response data to be corrupted
                    self.notify_reader_to_close();
                    LoopAction::Break
                } else {
                    LoopAction::Continue
                }
            }
        }
    }

    async fn run_ftp_over_http(
        &mut self,
        clt_w: &mut HttpClientWriter<CDW>,
        mut clt_r: HttpClientReader<CDR>,
        req: HttpProxyRequest<CDR>,
        task_notes: ServerTaskNotes,
    ) -> LoopAction {
        let mut ftp_task = FtpOverHttpTask::new(&self.ctx, &req, task_notes);
        ftp_task.run(&mut clt_r, clt_w).await;
        if ftp_task.should_close() {
            // close read end
            let _ = req.stream_sender.try_send(None);
            LoopAction::Break
        } else {
            // reopen read end
            if req.stream_sender.try_send(Some(clt_r)).is_err() {
                // read end has closed, impossible as reader should be waiting this channel
                LoopAction::Break
            } else {
                LoopAction::Continue
            }
        }
    }

    /// notify reader to close while it's not closed and not in waiting writer status.
    /// always use the req.stream_sender.send(None) when possible.
    fn notify_reader_to_close(&mut self) {
        self.task_queue.close(); // may be deleted as the writer will be dropped later
    }
}
