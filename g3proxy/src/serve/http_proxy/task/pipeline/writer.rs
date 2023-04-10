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

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use ahash::AHashMap;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

use g3_io_ext::{ArcLimitedWriterStats, LimitedWriter};
use g3_types::auth::{UserAuthError, Username};
use g3_types::net::{HttpAuth, HttpBasicAuth, HttpHeaderMap};
use g3_types::route::EgressPathSelection;

use super::protocol::{HttpClientReader, HttpClientWriter, HttpProxyRequest, HttpProxySubProtocol};
use super::{
    CommonTaskContext, FtpOverHttpTask, HttpProxyCltWrapperStats, HttpProxyConnectTask,
    HttpProxyForwardTask, HttpProxyPipelineStats, HttpProxyUntrustedTask,
};
use crate::auth::{UserContext, UserGroup, UserRequestStats};
use crate::config::server::ServerConfig;
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
    passed_users: AHashMap<Username, UserData>,
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
        let clt_w = LimitedWriter::new(
            write_half,
            limit_config.shift_millis,
            limit_config.max_south,
            Arc::clone(&clt_w_stats),
        );
        HttpProxyPipelineWriterTask {
            ctx: Arc::clone(ctx),
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
        use std::collections::hash_map::Entry;

        if let Some(user_group) = &self.user_group {
            match &req.inner.auth_info {
                HttpAuth::None => Err(UserAuthError::NoUserSupplied),
                HttpAuth::Basic(HttpBasicAuth {
                    username, password, ..
                }) => match user_group.get_user(username.as_original()) {
                    Some((user, user_type)) => {
                        let mut user_ctx = UserContext::new(
                            user,
                            user_type,
                            self.ctx.server_config.name(),
                            self.ctx.server_stats.extra_tags(),
                        );
                        user_ctx.check_password(password.as_original())?;
                        user_ctx.check_in_site(
                            self.ctx.server_config.name(),
                            self.ctx.server_stats.extra_tags(),
                            &req.upstream,
                        );

                        match self.req_count.passed_users.entry(username.clone()) {
                            Entry::Occupied(entry) => {
                                user_ctx.mark_reused_client_connection();
                                entry.into_mut().count += 1;
                            }
                            Entry::Vacant(entry) => {
                                let req_stats = user_ctx.req_stats().clone();
                                req_stats.conn_total.add_http();
                                req_stats.l7_conn_alive.inc_http();
                                let site_req_stats =
                                    if let Some(site_req_stats) = user_ctx.site_req_stats() {
                                        site_req_stats.conn_total.add_http();
                                        site_req_stats.l7_conn_alive.inc_http();
                                        Some(Arc::clone(site_req_stats))
                                    } else {
                                        None
                                    };
                                entry.insert(UserData {
                                    req_stats,
                                    site_req_stats,
                                    count: 1,
                                });
                            }
                        }

                        Ok(Some(user_ctx))
                    }
                    None => Err(UserAuthError::NoSuchUser),
                },
            }
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
                    if !self.ctx.server_config.no_early_error_reply {
                        if let Some(stream_w) = &mut self.stream_writer {
                            let _ = rsp.reply_err_to_request(stream_w).await;
                        }
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

    fn get_egress_path_selection(&self, headers: &mut HttpHeaderMap) -> EgressPathSelection {
        if let Some(header) = &self.ctx.server_config.egress_path_selection_header {
            // check and remove the custom header
            if let Some(value) = headers.remove(header) {
                return EgressPathSelection::from_str(value.to_str()).unwrap_or_default();
            }
        }
        EgressPathSelection::Default
    }

    async fn run(
        &mut self,
        mut req: HttpProxyRequest<CDR>,
        user_ctx: Option<UserContext>,
    ) -> LoopAction {
        let path_selection = self.get_egress_path_selection(&mut req.inner.end_to_end_headers);
        let task_notes = ServerTaskNotes::new(
            self.ctx.worker_id,
            self.ctx.tcp_client_addr,
            self.ctx.tcp_server_addr,
            user_ctx,
            req.time_accepted.elapsed(),
            path_selection,
        );

        let forward_capability = self
            .forward_context
            .check_in_final_escaper(&task_notes, &req.upstream)
            .await;
        let remote_protocol = match req.client_protocol {
            HttpProxySubProtocol::TcpConnect => HttpProxySubProtocol::TcpConnect,
            HttpProxySubProtocol::HttpForward => HttpProxySubProtocol::HttpForward,
            HttpProxySubProtocol::HttpsForward => {
                if forward_capability.forward_https() {
                    HttpProxySubProtocol::HttpForward
                } else {
                    HttpProxySubProtocol::HttpsForward
                }
            }
            HttpProxySubProtocol::FtpOverHttp => {
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
                    let mut connect_task = HttpProxyConnectTask::new(&self.ctx, &req, task_notes);
                    connect_task.connect_to_upstream(&mut stream_w).await;
                    if connect_task.back_to_http() {
                        // reopen write end
                        self.stream_writer = Some(stream_w);
                        // reopen read end
                        if req.stream_sender.send(Some(stream_r)).await.is_err() {
                            // read end has closed, impossible as reader should be waiting this channel
                            LoopAction::Break
                        } else {
                            LoopAction::Continue
                        }
                    } else {
                        // close read end
                        let _ = req.stream_sender.send(None).await;
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
                        .run_forward(&mut stream_w, req, task_notes, remote_protocol)
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
        stream_w.reset_limit(limit_config.shift_millis, limit_config.max_south);
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
                        let _ = req.stream_sender.send(None).await;
                        LoopAction::Break
                    } else {
                        // reopen read end
                        if req.stream_sender.send(clt_r).await.is_err() {
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
        remote_protocol: HttpProxySubProtocol,
    ) -> LoopAction {
        let is_https = match remote_protocol {
            HttpProxySubProtocol::HttpForward => false,
            HttpProxySubProtocol::HttpsForward => true,
            _ => unreachable!(),
        };

        match req.body_reader.take() {
            Some(stream_r) => {
                // we have a body, or we need to close the connection
                // we may need to send stream_r back if we have a body
                let mut forward_task =
                    HttpProxyForwardTask::new(&self.ctx, &req, is_https, task_notes);
                let mut clt_r = Some(stream_r);
                forward_task
                    .run(&mut clt_r, clt_w, &mut self.forward_context)
                    .await;
                if forward_task.should_close() {
                    // close read end
                    let _ = req.stream_sender.send(None).await;
                    LoopAction::Break
                } else {
                    // reopen read end
                    if req.stream_sender.send(clt_r).await.is_err() {
                        // read end has closed, impossible as reader should be waiting this channel
                        LoopAction::Break
                    } else {
                        LoopAction::Continue
                    }
                }
            }
            None => {
                // no body, and the connection is expected to keep alive from the client side
                let mut forward_task =
                    HttpProxyForwardTask::new(&self.ctx, &req, is_https, task_notes);
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
            let _ = req.stream_sender.send(None).await;
            LoopAction::Break
        } else {
            // reopen read end
            if req.stream_sender.send(Some(clt_r)).await.is_err() {
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
        self.task_queue.close(); // may be deleted as the writer will dropped later
    }
}
