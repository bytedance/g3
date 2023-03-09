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

use bytes::Bytes;
use chrono::{DateTime, Utc};
use h2::client::SendRequest;
use h2::server::SendResponse;
use h2::{Reason, RecvStream, SendStream, StreamId};
use http::{Request, Response};
use tokio::time::Instant;

use super::H2StreamTransferError;
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;

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
}

impl<'a, SC: ServerConfig> ExchangeHead<'a, SC> {
    fn new(ctx: &'a StreamInspectContext<SC>, http_notes: &'a mut HttpForwardTaskNotes) -> Self {
        ExchangeHead {
            ctx,
            ups_stream_id: None,
            send_error_response: false,
            http_notes,
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
        let ups_req = Request::from_parts(parts, ());

        let http_config = self.ctx.h2_interception();

        let mut ups_send_req =
            match tokio::time::timeout(http_config.upstream_stream_open_timeout, h2s.ready()).await
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

        let (ups_response_fut, ups_w) = ups_send_req
            .send_request(ups_req, false)
            .map_err(H2StreamTransferError::RequestHeadSendFailed)?;
        self.ups_stream_id = Some(ups_response_fut.stream_id());
        self.http_notes.mark_req_send_hdr();

        let ups_rsp =
            match tokio::time::timeout(http_config.rsp_head_recv_timeout, ups_response_fut).await {
                Ok(Ok(d)) => {
                    self.http_notes.mark_rsp_recv_hdr();
                    d
                }
                Ok(Err(e)) => return Err(H2StreamTransferError::ResponseHeadRecvFailed(e)),
                Err(_) => return Err(H2StreamTransferError::ResponseHeadRecvTimeout),
            };
        let (parts, ups_r) = ups_rsp.into_parts();
        let ups_rsp = Response::from_parts(parts, ());

        self.send_error_response = false;
        self.http_notes.origin_status = ups_rsp.status().as_u16();

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
}
