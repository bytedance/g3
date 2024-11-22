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

use std::time::Duration;

use anyhow::anyhow;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use h2::client::SendRequest;
use h2::server::SendResponse;
use h2::{Reason, RecvStream, SendStream, StreamId};
use http::{Request, Response, Version};
use tokio::time::Instant;

use g3_h2::{H2BodyTransfer, H2StreamBodyTransferError, H2StreamFromChunkedTransferError};
use g3_icap_client::reqmod::h1::HttpAdapterErrorResponse;
use g3_icap_client::reqmod::h2::{
    H2RequestAdapter, ReqmodAdaptationMidState, ReqmodAdaptationRunState,
    ReqmodRecvHttpResponseBody,
};
use g3_types::net::WebSocketNotes;

use super::H2StreamTransferError;
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;
use crate::serve::ServerIdleChecker;

mod standard;
pub(super) use standard::H2ConnectTask;

mod extended;
pub(super) use extended::H2ExtendedConnectTask;

struct HttpForwardTaskNotes {
    ready_time: Duration,
    rsp_status: u16,
    origin_status: u16,
    started_ins: Instant,
    started_datetime: DateTime<Utc>,
    dur_req_send_hdr: Duration,
    dur_rsp_recv_hdr: Duration,
}

impl Default for HttpForwardTaskNotes {
    fn default() -> Self {
        HttpForwardTaskNotes {
            ready_time: Duration::default(),
            rsp_status: 0,
            origin_status: 0,
            started_datetime: Utc::now(),
            started_ins: Instant::now(),
            dur_req_send_hdr: Duration::default(),
            dur_rsp_recv_hdr: Duration::default(),
        }
    }
}

impl HttpForwardTaskNotes {
    pub(crate) fn mark_stream_ready(&mut self) {
        self.ready_time = self.started_ins.elapsed();
    }

    pub(crate) fn mark_req_send_hdr(&mut self) {
        self.dur_req_send_hdr = self.started_ins.elapsed();
    }

    pub(crate) fn mark_rsp_recv_hdr(&mut self) {
        self.dur_rsp_recv_hdr = self.started_ins.elapsed();
    }
}

struct ExchangeHead<'a, SC: ServerConfig> {
    ctx: &'a StreamInspectContext<SC>,
    ups_stream_id: Option<StreamId>,
    send_error_response: bool,
    http_notes: &'a mut HttpForwardTaskNotes,
    ws_notes: Option<&'a mut WebSocketNotes>,
}

impl<'a, SC: ServerConfig> ExchangeHead<'a, SC> {
    fn new(ctx: &'a StreamInspectContext<SC>, http_notes: &'a mut HttpForwardTaskNotes) -> Self {
        ExchangeHead {
            ctx,
            ups_stream_id: None,
            send_error_response: false,
            http_notes,
            ws_notes: None,
        }
    }

    fn new_websocket(
        ctx: &'a StreamInspectContext<SC>,
        http_notes: &'a mut HttpForwardTaskNotes,
        ws_notes: &'a mut WebSocketNotes,
    ) -> Self {
        ExchangeHead {
            ctx,
            ups_stream_id: None,
            send_error_response: false,
            http_notes,
            ws_notes: Some(ws_notes),
        }
    }

    async fn run(
        &mut self,
        clt_req: Request<RecvStream>,
        mut clt_send_rsp: SendResponse<Bytes>,
        h2s: SendRequest<Bytes>,
    ) -> Result<
        Option<(RecvStream, SendStream<Bytes>, RecvStream, SendStream<Bytes>)>,
        H2StreamTransferError,
    > {
        match self.do_run(clt_req, &mut clt_send_rsp, h2s).await {
            Ok(d) => Ok(d),
            Err(e) => {
                if self.send_error_response {
                    if let Some(rsp) = e.build_reply() {
                        let rsp_status = rsp.status().as_u16();
                        if clt_send_rsp.send_response(rsp, true).is_ok() {
                            self.http_notes.rsp_status = rsp_status;
                        }
                    }
                }
                Err(e)
            }
        }
    }

    async fn do_run(
        &mut self,
        clt_req: Request<RecvStream>,
        clt_send_rsp: &mut SendResponse<Bytes>,
        h2s: SendRequest<Bytes>,
    ) -> Result<
        Option<(RecvStream, SendStream<Bytes>, RecvStream, SendStream<Bytes>)>,
        H2StreamTransferError,
    > {
        let (parts, clt_r) = clt_req.into_parts();

        let http_config = self.ctx.h2_interception();

        let ups_send_req =
            match tokio::time::timeout(http_config.upstream_stream_open_timeout, h2s.ready()).await
            {
                Ok(Ok(d)) => {
                    self.http_notes.mark_stream_ready();
                    d
                }
                Ok(Err(e)) => {
                    let reason = e.reason().unwrap_or(Reason::REFUSED_STREAM);
                    clt_send_rsp.send_reset(reason);
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
                    self.ctx.h2_rsp_hdr_recv_timeout(),
                    true,
                    self.ctx.idle_checker(),
                )
                .await
            {
                Ok(mut adapter) => {
                    let mut adaptation_state =
                        ReqmodAdaptationRunState::new(self.http_notes.started_ins);
                    adapter.set_client_addr(self.ctx.task_notes.client_addr);
                    if let Some(username) = self.ctx.raw_user_name() {
                        adapter.set_client_username(username.clone());
                    }
                    return self
                        .forward_with_adaptation(
                            ups_send_req,
                            ups_req,
                            clt_r,
                            clt_send_rsp,
                            adapter,
                            &mut adaptation_state,
                        )
                        .await;
                }
                Err(e) => {
                    if !reqmod.bypass() {
                        return Err(H2StreamTransferError::InternalAdapterError(e));
                    }
                }
            }
        }

        self.send_request(ups_send_req, ups_req, clt_r, clt_send_rsp)
            .await
    }

    async fn forward_with_adaptation(
        &mut self,
        ups_send_req: SendRequest<Bytes>,
        ups_req: Request<()>,
        clt_r: RecvStream,
        clt_send_rsp: &mut SendResponse<Bytes>,
        icap_adapter: H2RequestAdapter<ServerIdleChecker>,
        adaptation_state: &mut ReqmodAdaptationRunState,
    ) -> Result<
        Option<(RecvStream, SendStream<Bytes>, RecvStream, SendStream<Bytes>)>,
        H2StreamTransferError,
    > {
        match icap_adapter.xfer_connect(adaptation_state, ups_req).await {
            Ok(ReqmodAdaptationMidState::OriginalRequest(orig_req)) => {
                self.send_request(ups_send_req, orig_req, clt_r, clt_send_rsp)
                    .await
            }
            Ok(ReqmodAdaptationMidState::AdaptedRequest(_http_req, final_req)) => {
                self.send_request(ups_send_req, final_req, clt_r, clt_send_rsp)
                    .await
            }
            Ok(ReqmodAdaptationMidState::HttpErrResponse(err_rsp, recv_body)) => {
                self.send_adaptation_error_response(clt_send_rsp, err_rsp, recv_body)
                    .await?;
                Ok(None)
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
        parts.headers = rsp.headers.into();
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
                H2StreamFromChunkedTransferError::SenderNotInSendState => {
                    H2StreamTransferError::ResponseBodyTransferFailed(
                        H2StreamBodyTransferError::SenderNotInSendState,
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

    async fn send_request(
        &mut self,
        mut ups_send_req: SendRequest<Bytes>,
        ups_req: Request<()>,
        clt_r: RecvStream,
        clt_send_rsp: &mut SendResponse<Bytes>,
    ) -> Result<
        Option<(RecvStream, SendStream<Bytes>, RecvStream, SendStream<Bytes>)>,
        H2StreamTransferError,
    > {
        let (ups_response_fut, ups_w) = ups_send_req
            .send_request(ups_req, false)
            .map_err(H2StreamTransferError::RequestHeadSendFailed)?;
        self.ups_stream_id = Some(ups_response_fut.stream_id());
        self.http_notes.mark_req_send_hdr();

        match tokio::time::timeout(self.ctx.h2_rsp_hdr_recv_timeout(), ups_response_fut).await {
            Ok(Ok(ups_rsp)) => {
                self.http_notes.mark_rsp_recv_hdr();
                self.send_response(ups_rsp, ups_w, clt_r, clt_send_rsp)
                    .await
            }
            Ok(Err(e)) => Err(H2StreamTransferError::ResponseHeadRecvFailed(e)),
            Err(_) => Err(H2StreamTransferError::ResponseHeadRecvTimeout),
        }
    }

    async fn send_response(
        &mut self,
        ups_rsp: Response<RecvStream>,
        ups_w: SendStream<Bytes>,
        clt_r: RecvStream,
        clt_send_rsp: &mut SendResponse<Bytes>,
    ) -> Result<
        Option<(RecvStream, SendStream<Bytes>, RecvStream, SendStream<Bytes>)>,
        H2StreamTransferError,
    > {
        self.send_error_response = false;
        self.http_notes.origin_status = ups_rsp.status().as_u16();

        if ups_rsp.status().is_success() {
            self.send_ok_response(ups_rsp, ups_w, clt_r, clt_send_rsp)
                .await
        } else {
            self.send_err_response(ups_rsp, clt_send_rsp).await?;
            Ok(None)
        }
    }

    async fn send_ok_response(
        &mut self,
        ups_rsp: Response<RecvStream>,
        ups_w: SendStream<Bytes>,
        clt_r: RecvStream,
        clt_send_rsp: &mut SendResponse<Bytes>,
    ) -> Result<
        Option<(RecvStream, SendStream<Bytes>, RecvStream, SendStream<Bytes>)>,
        H2StreamTransferError,
    > {
        let (parts, ups_r) = ups_rsp.into_parts();

        if let Some(ws_notes) = self.ws_notes.take() {
            for (name, value) in &parts.headers {
                ws_notes.append_response_header(name, value);
            }
        }

        let ups_rsp = Response::from_parts(parts, ());

        if ups_r.is_end_stream() {
            let _ = clt_send_rsp
                .send_response(ups_rsp, true)
                .map_err(H2StreamTransferError::ResponseHeadSendFailed)?;
            self.http_notes.rsp_status = self.http_notes.origin_status;
            Ok(None)
        } else {
            let clt_w = clt_send_rsp
                .send_response(ups_rsp, false)
                .map_err(H2StreamTransferError::ResponseHeadSendFailed)?;
            self.http_notes.rsp_status = self.http_notes.origin_status;
            Ok(Some((clt_r, clt_w, ups_r, ups_w)))
        }
    }

    async fn send_err_response(
        &mut self,
        ups_rsp: Response<RecvStream>,
        clt_send_rsp: &mut SendResponse<Bytes>,
    ) -> Result<(), H2StreamTransferError> {
        let (parts, ups_r) = ups_rsp.into_parts();
        let ups_rsp = Response::from_parts(parts, ());

        if ups_r.is_end_stream() {
            let _ = clt_send_rsp
                .send_response(ups_rsp, true)
                .map_err(H2StreamTransferError::ResponseHeadSendFailed)?;
            self.http_notes.rsp_status = self.http_notes.origin_status;
        } else {
            let clt_send_stream = clt_send_rsp
                .send_response(ups_rsp, false)
                .map_err(H2StreamTransferError::ResponseHeadSendFailed)?;
            self.http_notes.rsp_status = self.http_notes.origin_status;

            let mut rsp_body_transfer = H2BodyTransfer::new(
                ups_r,
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
                            Ok(_) => break,
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
