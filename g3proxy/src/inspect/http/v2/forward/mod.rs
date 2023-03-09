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

use anyhow::anyhow;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use h2::client::SendRequest;
use h2::server::SendResponse;
use h2::{Reason, RecvStream, StreamId};
use http::{HeaderMap, Method, Request, Response, StatusCode, Uri, Version};
use slog::slog_info;
use tokio::time::Instant;

use g3_daemon::log::types::{LtDateTime, LtDuration, LtUuid};
use g3_h2::{H2StreamBodyTransferError, H2StreamFromChunkedTransferError, RequestExt};
use g3_icap_client::reqmod::h2::{
    H2RequestAdapter, HttpAdapterErrorResponse, ReqmodAdaptationEndState, ReqmodAdaptationRunState,
    ReqmodRecvHttpResponseBody,
};
use g3_icap_client::respmod::h2::{
    H2ResponseAdapter, RespmodAdaptationEndState, RespmodAdaptationRunState,
};

use super::{H2BodyTransfer, H2ConcurrencyStats, H2StreamTransferError};
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;
use crate::log::types::{LtH2StreamId, LtHttpMethod, LtHttpUri};
use crate::serve::ServerIdleChecker;

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        slog_info!($obj.ctx.intercept_logger(), $($args)+;
            "intercept_type" => "H2StreamForward",
            "task_id" => LtUuid($obj.ctx.server_task_id()),
            "depth" => $obj.ctx.inspection_depth,
            "clt_stream" => LtH2StreamId(&$obj.clt_stream_id),
            "ups_stream" => $obj.ups_stream_id.as_ref().map(LtH2StreamId),
            "started_at" => LtDateTime(&$obj.http_notes.started_datetime),
            "method" => LtHttpMethod(&$obj.http_notes.method),
            "uri" => LtHttpUri::new(&$obj.http_notes.uri, $obj.ctx.log_uri_max_chars()),
            "ready_time" => LtDuration($obj.http_notes.ready_time),
            "rsp_status" => $obj.http_notes.rsp_status,
            "origin_status" => $obj.http_notes.origin_status,
            "dur_req_send_hdr" => LtDuration($obj.http_notes.dur_req_send_hdr),
            "dur_req_send_all" => LtDuration($obj.http_notes.dur_req_send_all),
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
    dur_req_send_hdr: Duration,
    dur_req_send_all: Duration,
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
            dur_req_send_hdr: Duration::default(),
            dur_req_send_all: Duration::default(),
            dur_rsp_recv_hdr: Duration::default(),
            dur_rsp_recv_all: Duration::default(),
        }
    }

    pub(crate) fn mark_stream_ready(&mut self) {
        self.ready_time = self.started_ins.elapsed();
    }

    pub(crate) fn mark_req_send_hdr(&mut self) {
        self.dur_req_send_hdr = self.started_ins.elapsed();
    }

    pub(crate) fn mark_req_no_body(&mut self) {
        self.dur_req_send_all = self.dur_req_send_hdr;
    }

    pub(crate) fn mark_req_send_all(&mut self) {
        self.dur_req_send_all = self.started_ins.elapsed();
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

pub(crate) struct H2ForwardTask<SC: ServerConfig> {
    ctx: StreamInspectContext<SC>,
    clt_stream_id: StreamId,
    ups_stream_id: Option<StreamId>,
    send_error_response: bool,
    cstats: Arc<H2ConcurrencyStats>,
    http_notes: HttpForwardTaskNotes,
}

impl<SC> H2ForwardTask<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(crate) fn new(
        ctx: StreamInspectContext<SC>,
        clt_stream_id: StreamId,
        cstats: Arc<H2ConcurrencyStats>,
        req: &Request<RecvStream>,
    ) -> Self {
        let http_notes = HttpForwardTaskNotes::new(req.method().clone(), req.uri().clone());
        H2ForwardTask {
            ctx,
            clt_stream_id,
            ups_stream_id: None,
            send_error_response: false,
            cstats,
            http_notes,
        }
    }

    fn reply_task_err(&mut self, mut clt_send_rsp: SendResponse<Bytes>, e: &H2StreamTransferError) {
        if let Some(rsp) = e.build_reply() {
            let rsp_status = rsp.status().as_u16();
            if clt_send_rsp.send_response(rsp, true).is_ok() {
                self.http_notes.rsp_status = rsp_status;
            }
        }
    }

    fn reply_expectation_failed(
        &mut self,
        clt_send_rsp: &mut SendResponse<Bytes>,
    ) -> Result<(), H2StreamTransferError> {
        let response = Response::builder()
            .version(Version::HTTP_2)
            .status(StatusCode::EXPECTATION_FAILED)
            .body(())
            .map_err(|_| {
                H2StreamTransferError::InternalServerError(
                    "failed to build expectation failed error response",
                )
            })?;
        self.send_error_response = false;
        let rsp_code = response.status().as_u16();
        if clt_send_rsp.send_response(response, true).is_ok() {
            self.http_notes.rsp_status = rsp_code;
        }
        Ok(())
    }

    pub(crate) async fn forward(
        mut self,
        clt_req: Request<RecvStream>,
        mut clt_send_rsp: SendResponse<Bytes>,
        h2s: SendRequest<Bytes>,
    ) {
        if let Err(e) = self.do_forward(clt_req, &mut clt_send_rsp, h2s).await {
            if self.send_error_response {
                self.reply_task_err(clt_send_rsp, &e);
            }
            intercept_log!(self, "{e}");
        } else {
            intercept_log!(self, "finished");
        }
    }

    async fn do_forward(
        &mut self,
        clt_req: Request<RecvStream>,
        clt_send_rsp: &mut SendResponse<Bytes>,
        h2s: SendRequest<Bytes>,
    ) -> Result<(), H2StreamTransferError> {
        let (mut parts, clt_body) = clt_req.into_parts();
        if self.ctx.h2_interception().silent_drop_expect_header {
            // just drop the Expect header to avoid 100-continue response, which currently is not supported by h2
            parts.headers.remove(http::header::EXPECT);
        } else if parts.headers.contains_key(http::header::EXPECT) {
            return self.reply_expectation_failed(clt_send_rsp);
        }

        let ups_send_req = match tokio::time::timeout(
            self.ctx.h2_interception().upstream_stream_open_timeout,
            h2s.ready(),
        )
        .await
        {
            Ok(Ok(d)) => {
                self.http_notes.mark_stream_ready();
                d
            }
            Ok(Err(e)) => {
                clt_send_rsp.send_reset(Reason::REFUSED_STREAM);
                return Err(H2StreamTransferError::UpstreamStreamOpenFailed(e));
            }
            Err(_) => {
                clt_send_rsp.send_reset(Reason::REFUSED_STREAM);
                return Err(H2StreamTransferError::UpstreamStreamOpenTimeout);
            }
        };

        self.send_error_response = true;
        let ups_req = Request::from_parts(parts, ());

        if let Some(reqmod) = self.ctx.audit_handle.icap_reqmod_client() {
            match reqmod
                .h2_adapter(
                    self.ctx.server_config.limited_copy_config(),
                    self.ctx.h1_interception().body_line_max_len,
                    self.ctx.h2_interception().max_header_list_size as usize,
                    self.ctx.h2_interception().rsp_head_recv_timeout,
                    true,
                    self.ctx.idle_checker(),
                )
                .await
            {
                Ok(mut adapter) => {
                    let mut adaptation_state =
                        ReqmodAdaptationRunState::new(self.http_notes.started_ins);
                    adapter.set_client_addr(self.ctx.task_notes.client_addr);
                    if let Some(user) = self.ctx.user() {
                        adapter.set_client_username(user.name());
                    }
                    let r = self
                        .forward_with_adaptation(
                            ups_send_req,
                            ups_req,
                            clt_body,
                            clt_send_rsp,
                            adapter,
                            &mut adaptation_state,
                        )
                        .await;
                    if let Some(dur) = adaptation_state.dur_ups_send_header {
                        self.http_notes.dur_req_send_hdr = dur;
                    }
                    if let Some(dur) = adaptation_state.dur_ups_send_all {
                        self.http_notes.dur_req_send_all = dur;
                    }
                    if let Some(dur) = adaptation_state.dur_ups_recv_header {
                        self.http_notes.dur_rsp_recv_hdr = dur;
                    }
                    return r;
                }
                Err(e) => {
                    if !reqmod.bypass() {
                        return Err(H2StreamTransferError::InternalAdapterError(e));
                    }
                }
            }
        }

        self.forward_without_adaptation(ups_send_req, ups_req, clt_body, clt_send_rsp)
            .await
    }

    async fn forward_with_adaptation(
        &mut self,
        ups_send_req: SendRequest<Bytes>,
        ups_req: Request<()>,
        clt_body: RecvStream,
        clt_send_rsp: &mut SendResponse<Bytes>,
        icap_adapter: H2RequestAdapter<ServerIdleChecker>,
        adaptation_state: &mut ReqmodAdaptationRunState,
    ) -> Result<(), H2StreamTransferError> {
        let orig_req = ups_req.clone_header();

        match icap_adapter
            .xfer(adaptation_state, ups_req, clt_body, ups_send_req)
            .await
        {
            Ok(ReqmodAdaptationEndState::OriginalTransferred(ups_rsp)) => {
                self.send_response(
                    orig_req,
                    ups_rsp,
                    clt_send_rsp,
                    adaptation_state.take_respond_shared_headers(),
                )
                .await
            }
            Ok(ReqmodAdaptationEndState::AdaptedTransferred(_http_req, ups_rsp)) => {
                self.send_response(
                    orig_req,
                    ups_rsp,
                    clt_send_rsp,
                    adaptation_state.take_respond_shared_headers(),
                )
                .await
            }
            Ok(ReqmodAdaptationEndState::HttpErrResponse(err_rsp, recv_body)) => {
                self.send_adaptation_error_response(clt_send_rsp, err_rsp, recv_body)
                    .await
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn send_adaptation_error_response(
        &mut self,
        clt_send_rsp: &mut SendResponse<Bytes>,
        rsp: HttpAdapterErrorResponse,
        rsp_recv_body: Option<ReqmodRecvHttpResponseBody>,
    ) -> Result<(), H2StreamTransferError> {
        let response = Response::new(());
        let (mut parts, _) = response.into_parts();
        parts.version = Version::HTTP_2;
        parts.status = rsp.status;
        parts.headers = rsp.headers;
        let response = Response::from_parts(parts, ());

        self.send_error_response = false;
        let rsp_status = response.status().as_u16();
        if let Some(mut recv_body) = rsp_recv_body {
            let mut clt_send_stream = clt_send_rsp
                .send_response(response, false)
                .map_err(H2StreamTransferError::ResponseHeadSendFailed)?;
            self.http_notes.rsp_status = rsp_status;

            let body_transfer = recv_body.body_transfer(&mut clt_send_stream);
            body_transfer.await.map_err(|e| match e {
                H2StreamFromChunkedTransferError::ReadError(e) => {
                    H2StreamTransferError::InternalAdapterError(anyhow!(
                        "read http error response from adapter failed: {e:?}"
                    ))
                }
                H2StreamFromChunkedTransferError::SendDataFailed(e) => {
                    H2StreamTransferError::ResponseBodyTransferFailed(
                        H2StreamBodyTransferError::SendDataFailed(e),
                    )
                }
                H2StreamFromChunkedTransferError::SendTrailerFailed(e) => {
                    H2StreamTransferError::ResponseBodyTransferFailed(
                        H2StreamBodyTransferError::SendTrailersFailed(e),
                    )
                }
            })?;

            recv_body.save_connection().await;
        } else {
            clt_send_rsp
                .send_response(response, true)
                .map_err(H2StreamTransferError::ResponseHeadSendFailed)?;
            self.http_notes.rsp_status = rsp_status;
        }

        Ok(())
    }

    async fn forward_without_adaptation(
        &mut self,
        ups_send_req: SendRequest<Bytes>,
        ups_req: Request<()>,
        clt_body: RecvStream,
        clt_send_rsp: &mut SendResponse<Bytes>,
    ) -> Result<(), H2StreamTransferError> {
        if clt_body.is_end_stream() {
            self.forward_without_body(ups_send_req, ups_req, clt_send_rsp)
                .await
        } else {
            self.forward_with_body(ups_send_req, ups_req, clt_body, clt_send_rsp)
                .await
        }
    }

    async fn forward_without_body(
        &mut self,
        mut ups_send_req: SendRequest<Bytes>,
        ups_req: Request<()>,
        clt_send_rsp: &mut SendResponse<Bytes>,
    ) -> Result<(), H2StreamTransferError> {
        let orig_req = ups_req.clone_header();

        let send_push_promise = ups_req.method().eq(&Method::GET); // only forward push promise for GET

        let (mut ups_rsp_fut, _) = ups_send_req
            .send_request(ups_req, true)
            .map_err(H2StreamTransferError::RequestHeadSendFailed)?; // do not send REFUSED_STREAM, use the default rst in h2
        self.ups_stream_id = Some(ups_rsp_fut.stream_id());
        self.http_notes.mark_req_send_hdr();
        self.http_notes.mark_req_no_body();

        let ups_push = if send_push_promise {
            Some(ups_rsp_fut.push_promises())
        } else {
            None
        };

        // there shouldn't be 100 response in this case
        let ups_rsp = match tokio::time::timeout(
            self.ctx.h2_interception().rsp_head_recv_timeout,
            ups_rsp_fut,
        )
        .await
        {
            Ok(Ok(d)) => {
                self.http_notes.mark_rsp_recv_hdr();
                d
            }
            Ok(Err(e)) => return Err(H2StreamTransferError::ResponseHeadRecvFailed(e)),
            Err(_) => return Err(H2StreamTransferError::ResponseHeadRecvTimeout),
        };

        if let Some(mut ups_push) = ups_push {
            loop {
                match ups_push.push_promise().await {
                    Some(Ok(p)) => {
                        if super::push::push_request(
                            p,
                            clt_send_rsp,
                            &self.ctx,
                            self.cstats.clone(),
                        )
                        .await
                        .is_err()
                        {
                            break;
                        }
                    }
                    Some(Err(e)) => return Err(H2StreamTransferError::PushWaitError(e)),
                    None => break,
                }
            }
        }

        self.send_response(orig_req, ups_rsp, clt_send_rsp, None)
            .await
    }

    async fn forward_with_body(
        &mut self,
        mut ups_send_req: SendRequest<Bytes>,
        ups_req: Request<()>,
        clt_body: RecvStream,
        clt_send_rsp: &mut SendResponse<Bytes>,
    ) -> Result<(), H2StreamTransferError> {
        let orig_req = ups_req.clone_header();

        let (mut ups_rsp_fut, ups_send_stream) = ups_send_req
            .send_request(ups_req, false)
            .map_err(H2StreamTransferError::RequestHeadSendFailed)?; // do not send REFUSED_STREAM, use the default rst in h2
        self.ups_stream_id = Some(ups_rsp_fut.stream_id());
        self.http_notes.mark_req_send_hdr();

        let mut req_body_transfer = H2BodyTransfer::new(
            clt_body,
            ups_send_stream,
            self.ctx.server_config.limited_copy_config().yield_size(),
        );

        let idle_duration = self.ctx.server_config.task_idle_check_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;
        let max_idle_count = self.ctx.task_max_idle_count();

        let mut ups_rsp: Option<Response<RecvStream>> = None;

        loop {
            tokio::select! {
                biased;

                r = &mut req_body_transfer => {
                    match r {
                        Ok(_) => {
                            self.http_notes.mark_req_send_all();
                            break;
                        }
                        Err(e) => {
                            return Err(H2StreamTransferError::RequestBodyTransferFailed(e));
                        }
                    }
                }
                r = &mut ups_rsp_fut => {
                    match r {
                        Ok(rsp) => {
                            self.http_notes.mark_rsp_recv_hdr();
                            ups_rsp = Some(rsp);
                            break;
                        }
                        Err(e) => {
                            return Err(H2StreamTransferError::ResponseHeadRecvFailed(e));
                        }
                    }
                }
                _ = idle_interval.tick() => {
                    if req_body_transfer.is_idle() {
                        idle_count += 1;

                        if idle_count > max_idle_count {
                            return Err(H2StreamTransferError::Idle(idle_duration, idle_count));
                        }
                    } else {
                        idle_count = 0;

                        req_body_transfer.reset_active();
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

        if let Some(ups_rsp) = ups_rsp {
            self.send_response(orig_req, ups_rsp, clt_send_rsp, None)
                .await
        } else {
            let ups_rsp = match tokio::time::timeout(
                self.ctx.h2_interception().rsp_head_recv_timeout,
                ups_rsp_fut,
            )
            .await
            {
                Ok(Ok(d)) => {
                    self.http_notes.mark_rsp_recv_hdr();
                    d
                }
                Ok(Err(e)) => return Err(H2StreamTransferError::ResponseHeadRecvFailed(e)),
                Err(_) => return Err(H2StreamTransferError::ResponseHeadRecvTimeout),
            };

            self.send_response(orig_req, ups_rsp, clt_send_rsp, None)
                .await
        }
    }

    async fn send_response(
        &mut self,
        ups_req: Request<()>,
        ups_rsp: Response<RecvStream>,
        clt_send_rsp: &mut SendResponse<Bytes>,
        adaptation_respond_shared_headers: Option<HeaderMap>,
    ) -> Result<(), H2StreamTransferError> {
        let (parts, ups_body) = ups_rsp.into_parts();
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
                    adapter.set_respond_shared_headers(adaptation_respond_shared_headers);
                    let r = self
                        .send_response_with_adaptation(
                            &ups_req,
                            clt_rsp,
                            ups_body,
                            clt_send_rsp,
                            adapter,
                            &mut adaptation_state,
                        )
                        .await;
                    if let Some(dur) = adaptation_state.dur_ups_recv_all {
                        self.http_notes.dur_rsp_recv_all = dur;
                    }
                    if adaptation_state.clt_write_started {
                        self.send_error_response = false;
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

        self.send_response_without_adaptation(clt_rsp, ups_body, clt_send_rsp)
            .await
    }

    async fn send_response_with_adaptation(
        &mut self,
        ups_req: &Request<()>,
        clt_rsp: Response<()>,
        ups_body: RecvStream,
        clt_send_rsp: &mut SendResponse<Bytes>,
        icap_adapter: H2ResponseAdapter<ServerIdleChecker>,
        adaptation_state: &mut RespmodAdaptationRunState,
    ) -> Result<(), H2StreamTransferError> {
        let rsp_code = clt_rsp.status().as_u16();
        match icap_adapter
            .xfer(adaptation_state, ups_req, clt_rsp, ups_body, clt_send_rsp)
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

    async fn send_response_without_adaptation(
        &mut self,
        clt_rsp: Response<()>,
        ups_body: RecvStream,
        clt_send_rsp: &mut SendResponse<Bytes>,
    ) -> Result<(), H2StreamTransferError> {
        self.send_error_response = false;

        if ups_body.is_end_stream() {
            self.http_notes.mark_rsp_no_body();
            let _ = clt_send_rsp
                .send_response(clt_rsp, true)
                .map_err(H2StreamTransferError::ResponseHeadSendFailed)?;
            self.http_notes.rsp_status = self.http_notes.origin_status;
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
