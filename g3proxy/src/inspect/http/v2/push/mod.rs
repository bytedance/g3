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

use bytes::Bytes;
use chrono::{DateTime, Utc};
use h2::client::{PushPromise, PushedResponseFuture};
use h2::server::{SendPushedResponse, SendResponse};
use h2::{RecvStream, StreamId};
use http::{Method, Request, Response, Uri};
use slog::slog_info;
use tokio::time::Instant;

use g3_h2::RequestExt;
use g3_icap_client::respmod::h2::{
    H2ResponseAdapter, RespmodAdaptationEndState, RespmodAdaptationRunState,
};
use g3_slog_types::{LtDateTime, LtDuration, LtH2StreamId, LtHttpMethod, LtHttpUri, LtUuid};

use super::{H2BodyTransfer, H2ConcurrencyStats, H2StreamTransferError};
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;
use crate::serve::ServerIdleChecker;

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        slog_info!($obj.ctx.intercept_logger(), $($args)+;
            "intercept_type" => "H2StreamPush",
            "task_id" => LtUuid($obj.ctx.server_task_id()),
            "depth" => $obj.ctx.inspection_depth,
            "origin_clt_stream" => LtH2StreamId(&$obj.origin_clt_stream),
            "ups_stream" => LtH2StreamId(&$obj.ups_stream_id),
            "clt_stream" => $obj.clt_stream_id.as_ref().map(LtH2StreamId),
            "started_at" => LtDateTime(&$obj.http_notes.started_datetime),
            "method" => LtHttpMethod(&$obj.http_notes.method),
            "uri" => LtHttpUri::new(&$obj.http_notes.uri, $obj.ctx.log_uri_max_chars()),
            "ready_time" => LtDuration($obj.http_notes.ready_time),
            "rsp_status" => $obj.http_notes.rsp_status,
            "origin_status" => $obj.http_notes.origin_status,
            "dur_rsp_recv_hdr" => LtDuration($obj.http_notes.dur_rsp_recv_hdr),
            "dur_rsp_recv_all" => LtDuration($obj.http_notes.dur_rsp_recv_all),
        )
    };
}

struct HttpForwardTaskNotes {
    method: Method,
    uri: Uri,
    ready_time: Duration,
    rsp_status: u16,
    origin_status: u16,
    started_ins: Instant,
    started_datetime: DateTime<Utc>,
    dur_rsp_recv_hdr: Duration,
    dur_rsp_recv_all: Duration,
}

impl HttpForwardTaskNotes {
    fn new(method: Method, uri: Uri) -> Self {
        HttpForwardTaskNotes {
            method,
            uri,
            ready_time: Duration::default(),
            rsp_status: 0,
            origin_status: 0,
            started_datetime: Utc::now(),
            started_ins: Instant::now(),
            dur_rsp_recv_hdr: Duration::default(),
            dur_rsp_recv_all: Duration::default(),
        }
    }

    pub(crate) fn mark_stream_ready(&mut self) {
        self.ready_time = self.started_ins.elapsed();
    }

    pub(crate) fn mark_rsp_recv_hdr(&mut self) {
        self.dur_rsp_recv_hdr = self.started_ins.elapsed();
    }

    pub(crate) fn mark_rsp_no_body(&mut self) {
        self.dur_rsp_recv_all = self.dur_rsp_recv_hdr;
    }

    pub(crate) fn mark_rsp_recv_all(&mut self) {
        self.dur_rsp_recv_all = self.started_ins.elapsed();
    }
}

pub(super) async fn push_request<SC>(
    promise: PushPromise,
    clt_send_rsp: &mut SendResponse<Bytes>,
    ctx: &StreamInspectContext<SC>,
    cstats: Arc<H2ConcurrencyStats>,
) -> Result<(), ()>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    let (req, f) = promise.into_parts();
    let http_notes = HttpForwardTaskNotes::new(req.method().clone(), req.uri().clone());
    let mut push_task = H2PushTask {
        ctx: ctx.clone(),
        origin_clt_stream: clt_send_rsp.stream_id(),
        ups_request: req.clone_header(),
        ups_stream_id: f.stream_id(),
        clt_stream_id: None,
        http_notes,
    };
    match clt_send_rsp.push_request(req) {
        Ok(send_pushed_rsp) => {
            cstats.add_task();
            push_task.clt_stream_id = Some(send_pushed_rsp.stream_id());
            push_task.http_notes.mark_stream_ready();
            tokio::spawn(async move {
                push_task.push(f, send_pushed_rsp).await;
                cstats.del_task();
            });
            Ok(())
        }
        Err(e) => {
            intercept_log!(push_task, "push req error: {e}");
            Err(())
        }
    }
}

struct H2PushTask<SC: ServerConfig> {
    ctx: StreamInspectContext<SC>,
    origin_clt_stream: StreamId,
    ups_request: Request<()>,
    ups_stream_id: StreamId,
    clt_stream_id: Option<StreamId>,
    http_notes: HttpForwardTaskNotes,
}

impl<SC: ServerConfig> H2PushTask<SC> {
    async fn push(mut self, rsp_fut: PushedResponseFuture, send_rsp: SendPushedResponse<Bytes>) {
        if let Err(e) = self.do_push(rsp_fut, send_rsp).await {
            intercept_log!(self, "push rsp error: {e}");
        } else {
            intercept_log!(self, "finished");
        }
    }

    async fn do_push(
        &mut self,
        rsp_fut: PushedResponseFuture,
        mut clt_send_rsp: SendPushedResponse<Bytes>,
    ) -> Result<(), H2StreamTransferError> {
        let rsp =
            match tokio::time::timeout(self.ctx.h2_interception().rsp_head_recv_timeout, rsp_fut)
                .await
            {
                Ok(Ok(d)) => {
                    self.http_notes.mark_rsp_recv_hdr();
                    d
                }
                Ok(Err(e)) => return Err(H2StreamTransferError::ResponseHeadRecvFailed(e)),
                Err(_) => return Err(H2StreamTransferError::ResponseHeadRecvTimeout),
            };

        let (parts, ups_body) = rsp.into_parts();
        let clt_rsp = Response::from_parts(parts, ());

        self.http_notes.origin_status = clt_rsp.status().as_u16();

        if let Some(respmod) = self.ctx.audit_handle.icap_respmod_client() {
            match respmod
                .h2_adapter(
                    self.ctx.server_config.limited_copy_config(),
                    self.ctx.h1_interception().body_line_max_len,
                    self.ctx.h2_interception().max_header_list_size as usize,
                    self.ctx.idle_checker(),
                )
                .await
            {
                Ok(mut adapter) => {
                    let mut adaptation_state = RespmodAdaptationRunState::new(
                        self.http_notes.started_ins,
                        self.http_notes.dur_rsp_recv_hdr,
                    );
                    adapter.set_client_addr(self.ctx.task_notes.client_addr);
                    if let Some(user) = self.ctx.user() {
                        adapter.set_client_username(user.name());
                    }
                    let r = self
                        .push_with_adaptation(
                            clt_rsp,
                            ups_body,
                            &mut clt_send_rsp,
                            adapter,
                            &mut adaptation_state,
                        )
                        .await;
                    if let Some(dur) = adaptation_state.dur_ups_recv_all {
                        self.http_notes.dur_rsp_recv_all = dur;
                    }
                    return r;
                }
                Err(e) => {
                    if !respmod.bypass() {
                        return Err(H2StreamTransferError::InternalAdapterError(e));
                    }
                }
            }
        }

        self.push_without_adaptation(clt_rsp, ups_body, &mut clt_send_rsp)
            .await
    }

    async fn push_with_adaptation(
        &mut self,
        clt_rsp: Response<()>,
        ups_body: RecvStream,
        clt_send_rsp: &mut SendPushedResponse<Bytes>,
        icap_adapter: H2ResponseAdapter<ServerIdleChecker>,
        adaptation_state: &mut RespmodAdaptationRunState,
    ) -> Result<(), H2StreamTransferError> {
        let rsp_code = clt_rsp.status().as_u16();
        match icap_adapter
            .xfer(
                adaptation_state,
                &self.ups_request,
                clt_rsp,
                ups_body,
                clt_send_rsp,
            )
            .await
        {
            Ok(RespmodAdaptationEndState::OriginalTransferred) => {
                self.http_notes.rsp_status = rsp_code;
                Ok(())
            }
            Ok(RespmodAdaptationEndState::AdaptedTransferred(_rsp)) => {
                self.http_notes.rsp_status = rsp_code;
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn push_without_adaptation(
        &mut self,
        clt_rsp: Response<()>,
        ups_body: RecvStream,
        clt_send_rsp: &mut SendPushedResponse<Bytes>,
    ) -> Result<(), H2StreamTransferError> {
        if ups_body.is_end_stream() {
            let _ = clt_send_rsp
                .send_response(clt_rsp, true)
                .map_err(H2StreamTransferError::ResponseHeadSendFailed)?;
            self.http_notes.rsp_status = self.http_notes.origin_status;
            self.http_notes.mark_rsp_no_body();
        } else {
            let clt_send_stream = clt_send_rsp
                .send_response(clt_rsp, false)
                .map_err(H2StreamTransferError::ResponseHeadSendFailed)?;
            self.http_notes.rsp_status = self.http_notes.origin_status;
            let mut rsp_body_transfer = H2BodyTransfer::new(
                ups_body,
                clt_send_stream,
                self.ctx.server_config.limited_copy_config().yield_size(),
            );

            let idle_duration = self.ctx.server_config.task_idle_check_duration();
            let mut idle_interval =
                tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
            let mut idle_count = 0;
            let max_idle_count = self.ctx.task_max_idle_count();

            loop {
                tokio::select! {
                    biased;

                    r = &mut rsp_body_transfer => {
                        match r {
                            Ok(_) => {
                                self.http_notes.mark_rsp_recv_all();
                                break;
                            },
                            Err(e) => return Err(H2StreamTransferError::ResponseBodyTransferFailed(e)),
                        }
                    }
                    _ = idle_interval.tick() => {
                        if rsp_body_transfer.is_idle() {
                            idle_count += 1;

                            if idle_count > max_idle_count {
                                return Err(H2StreamTransferError::Idle(idle_duration, idle_count));
                            }
                        } else {
                            idle_count = 0;

                            rsp_body_transfer.reset_active();
                        }

                        if self.ctx.belongs_to_blocked_user() {
                            return Err(H2StreamTransferError::CanceledAsUserBlocked);
                        }

                        if self.ctx.server_force_quit() {
                            return Err(H2StreamTransferError::CanceledAsServerQuit)
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
