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
use slog::slog_info;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::time::Instant;

use g3_daemon::log::types::{LtDateTime, LtDuration, LtUpstreamAddr, LtUuid};
use g3_http::client::HttpTransparentResponse;
use g3_http::server::{HttpTransparentRequest, UriExt};
use g3_types::net::UpstreamAddr;

use super::{HttpRequest, HttpRequestIo, HttpResponseIo};
use crate::config::server::ServerConfig;
use crate::inspect::{BoxAsyncRead, BoxAsyncWrite, StreamInspectContext, StreamInspection};
use crate::module::http_forward::HttpProxyClientResponse;
use crate::serve::{ServerTaskError, ServerTaskResult};

macro_rules! intercept_log {
    ($obj:tt, $r:expr, $($args:tt)+) => {
        slog_info!($obj.ctx.intercept_logger(), $($args)+;
            "intercept_type" => "HttpConnect",
            "task_id" => LtUuid($obj.ctx.server_task_id()),
            "depth" => $obj.ctx.inspection_depth,
            "request_id" => $obj.req_id,
            "next_upstream" => $r.as_ref().map(LtUpstreamAddr),
            "received_at" => LtDateTime(&$obj.http_notes.receive_datetime),
            "rsp_status" => $obj.http_notes.rsp_status,
            "origin_status" => $obj.http_notes.origin_status,
            "dur_req_send_hdr" => LtDuration($obj.http_notes.dur_req_send_hdr),
            "dur_req_pipeline" => LtDuration($obj.http_notes.dur_req_pipeline),
            "dur_rsp_recv_hdr" => LtDuration($obj.http_notes.dur_rsp_recv_hdr),
        )
    };
}

struct HttpForwardTaskNotes {
    rsp_status: u16,
    origin_status: u16,
    receive_ins: Instant,
    receive_datetime: DateTime<Utc>,
    dur_req_send_hdr: Duration,
    dur_req_pipeline: Duration,
    dur_rsp_recv_hdr: Duration,
}

impl HttpForwardTaskNotes {
    fn new(
        datetime_received: DateTime<Utc>,
        time_received: Instant,
        dur_req_send_hdr: Duration,
    ) -> Self {
        let dur_req_pipeline = time_received.elapsed();
        HttpForwardTaskNotes {
            rsp_status: 0,
            origin_status: 0,
            receive_ins: time_received,
            receive_datetime: datetime_received,
            dur_req_send_hdr,
            dur_req_pipeline,
            dur_rsp_recv_hdr: Duration::default(),
        }
    }

    pub(crate) fn mark_rsp_recv_hdr(&mut self) {
        self.dur_rsp_recv_hdr = self.receive_ins.elapsed();
    }
}

pub(super) struct H1ConnectTask<SC: ServerConfig> {
    ctx: StreamInspectContext<SC>,
    req: HttpTransparentRequest,
    req_id: usize,
    send_error_response: bool,
    should_close: bool,
    http_notes: HttpForwardTaskNotes,
}

impl<SC> H1ConnectTask<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(super) fn new(ctx: StreamInspectContext<SC>, req: HttpRequest, req_id: usize) -> Self {
        let http_notes = HttpForwardTaskNotes::new(
            req.datetime_received,
            req.time_received,
            req.dur_req_send_hdr,
        );
        H1ConnectTask {
            ctx,
            req: req.inner,
            req_id,
            send_error_response: true,
            should_close: false,
            http_notes,
        }
    }

    #[inline]
    pub(super) fn should_close(&self) -> bool {
        self.should_close
    }

    async fn reply_task_err<CW>(&mut self, e: &ServerTaskError, clt_w: &mut CW)
    where
        CW: AsyncWrite + Unpin,
    {
        let rsp = HttpProxyClientResponse::from_task_err(e, self.req.version, self.should_close);

        if let Some(rsp) = rsp {
            if rsp.should_close() {
                self.should_close = true;
            }

            if rsp.reply_err_to_request(clt_w).await.is_err() {
                self.should_close = true;
            } else {
                self.http_notes.rsp_status = rsp.status();
            }
        }
    }

    pub(super) async fn recv_connect<CW, UR>(
        &mut self,
        rsp_io: &mut HttpResponseIo<UR, CW>,
    ) -> Option<UpstreamAddr>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        match self.do_recv_connect(rsp_io).await {
            Ok(v) => {
                intercept_log!(self, &v, "ok");
                v
            }
            Err(e) => {
                if self.send_error_response {
                    self.reply_task_err(&e, &mut rsp_io.clt_w).await;
                }
                intercept_log!(self, &None, "{e}");
                None
            }
        }
    }

    async fn do_recv_connect<CW, UR>(
        &mut self,
        rsp_io: &mut HttpResponseIo<UR, CW>,
    ) -> ServerTaskResult<Option<UpstreamAddr>>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        let http_config = self.ctx.h1_interception();
        match tokio::time::timeout(
            http_config.rsp_head_recv_timeout,
            HttpTransparentResponse::parse(
                &mut rsp_io.ups_r,
                &self.req.method,
                self.req.keep_alive(),
                http_config.rsp_head_max_size,
            ),
        )
        .await
        {
            Ok(Ok((rsp, head_bytes))) => {
                self.send_error_response = false;
                self.http_notes.origin_status = rsp.code;
                self.http_notes.mark_rsp_recv_hdr();
                if !rsp.keep_alive() {
                    self.should_close = true;
                }
                self.handle_response(rsp, head_bytes, rsp_io).await
            }
            Ok(Err(e)) => Err(e.into()),
            Err(_) => Err(ServerTaskError::UpstreamAppTimeout(
                "timeout to receive response header",
            )),
        }
    }

    async fn handle_response<CW, UR>(
        &mut self,
        rsp: HttpTransparentResponse,
        rsp_head: Bytes,
        rsp_io: &mut HttpResponseIo<UR, CW>,
    ) -> ServerTaskResult<Option<UpstreamAddr>>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        rsp_io
            .clt_w
            .write_all(&rsp_head)
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)?;
        self.http_notes.rsp_status = self.http_notes.origin_status;

        if rsp.code >= 200 && rsp.code < 300 {
            let upstream = self
                .req
                .uri
                .get_upstream_with_default_port(443)
                .map_err(ServerTaskError::from)?;
            Ok(Some(upstream))
        } else {
            Ok(None)
        }
    }

    pub(super) fn into_connect(
        self,
        req_io: HttpRequestIo<BoxAsyncRead, BoxAsyncWrite>,
        rsp_io: HttpResponseIo<BoxAsyncRead, BoxAsyncWrite>,
        upstream: UpstreamAddr,
    ) -> StreamInspection<SC> {
        let (clt_r, clt_w, ups_r, ups_w) = super::convert_io(req_io, rsp_io);

        let mut stream_obj = crate::inspect::stream::StreamInspectObject::new(self.ctx, upstream);
        stream_obj.set_io(clt_r, clt_w, ups_r, ups_w);
        StreamInspection::StreamInspect(stream_obj)
    }
}
