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
use slog::slog_info;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::time::Instant;

use g3_http::client::HttpTransparentResponse;
use g3_http::server::{HttpTransparentRequest, UriExt};
use g3_http::{HttpBodyReader, HttpBodyType};
use g3_icap_client::reqmod::h1::{
    H1ReqmodAdaptationError, HttpAdapterErrorResponse, HttpRequestAdapter,
    HttpRequestUpstreamWriter, ReqmodAdaptationMidState, ReqmodAdaptationRunState,
    ReqmodRecvHttpResponseBody,
};
use g3_icap_client::reqmod::IcapReqmodClient;
use g3_io_ext::{LimitedCopy, LimitedCopyError};
use g3_slog_types::{LtDateTime, LtDuration, LtUpstreamAddr, LtUuid};
use g3_types::net::UpstreamAddr;

use super::{HttpRequest, HttpRequestIo, HttpRequestWriterForAdaptation, HttpResponseIo};
use crate::config::server::ServerConfig;
use crate::inspect::{BoxAsyncRead, BoxAsyncWrite, StreamInspectContext, StreamInspection};
use crate::module::http_forward::HttpProxyClientResponse;
use crate::serve::{ServerIdleChecker, ServerTaskError, ServerTaskResult};

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

    fn mark_ups_send_header(&mut self) {
        self.dur_req_send_hdr = self.receive_ins.elapsed();
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

    pub(super) async fn forward<CR, CW, UR, UW>(
        &mut self,
        req_io: &mut HttpRequestIo<CR, UW>,
        rsp_io: &mut HttpResponseIo<UR, CW>,
        reqmod_client: &IcapReqmodClient,
    ) -> Option<UpstreamAddr>
    where
        CR: AsyncRead + Send + Unpin,
        CW: AsyncWrite + Send + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Send + Unpin,
    {
        match self.do_forward(req_io, rsp_io, reqmod_client).await {
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

    pub(super) async fn recv_response<CW, UR>(
        &mut self,
        rsp_io: &mut HttpResponseIo<UR, CW>,
    ) -> Option<UpstreamAddr>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        match self.do_recv_response(rsp_io).await {
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

    async fn do_forward<CR, CW, UR, UW>(
        &mut self,
        req_io: &mut HttpRequestIo<CR, UW>,
        rsp_io: &mut HttpResponseIo<UR, CW>,
        reqmod_client: &IcapReqmodClient,
    ) -> ServerTaskResult<Option<UpstreamAddr>>
    where
        CR: AsyncRead + Send + Unpin,
        CW: AsyncWrite + Send + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Send + Unpin,
    {
        match reqmod_client
            .h1_adapter(
                self.ctx.server_config.limited_copy_config(),
                self.ctx.h1_interception().body_line_max_len,
                true,
                self.ctx.idle_checker(),
            )
            .await
        {
            Ok(mut adapter) => {
                adapter.set_client_addr(self.ctx.task_notes.client_addr);
                if let Some(username) = self.ctx.raw_user_name() {
                    adapter.set_client_username(username);
                }
                let mut adaptation_state =
                    ReqmodAdaptationRunState::new(self.http_notes.receive_ins);
                self.forward_with_adaptation(req_io, rsp_io, adapter, &mut adaptation_state)
                    .await
            }
            Err(e) => {
                if reqmod_client.bypass() {
                    self.send_request(None, req_io, rsp_io).await
                } else {
                    Err(ServerTaskError::InternalAdapterError(e))
                }
            }
        }
    }

    async fn forward_with_adaptation<CR, CW, UR, UW>(
        &mut self,
        req_io: &mut HttpRequestIo<CR, UW>,
        rsp_io: &mut HttpResponseIo<UR, CW>,
        icap_adapter: HttpRequestAdapter<ServerIdleChecker>,
        adaptation_state: &mut ReqmodAdaptationRunState,
    ) -> ServerTaskResult<Option<UpstreamAddr>>
    where
        CR: AsyncRead + Send + Unpin,
        CW: AsyncWrite + Send + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Send + Unpin,
    {
        match icap_adapter.xfer_connect(adaptation_state, &self.req).await {
            Ok(ReqmodAdaptationMidState::OriginalRequest) => {
                self.send_request(None, req_io, rsp_io).await
            }
            Ok(ReqmodAdaptationMidState::AdaptedRequest(final_req)) => {
                self.send_request(Some(final_req), req_io, rsp_io).await
            }
            Ok(ReqmodAdaptationMidState::HttpErrResponse(rsp, rsp_body)) => {
                self.send_adaptation_error_response(&mut rsp_io.clt_w, rsp, rsp_body)
                    .await?;
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn send_adaptation_error_response<W>(
        &mut self,
        clt_w: &mut W,
        rsp: HttpAdapterErrorResponse,
        rsp_recv_body: Option<ReqmodRecvHttpResponseBody>,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        self.should_close = true;

        let buf = rsp.serialize(self.should_close);
        self.send_error_response = false;
        clt_w
            .write_all(buf.as_ref())
            .await
            .map_err(ServerTaskError::ClientTcpWriteFailed)?;
        self.http_notes.rsp_status = rsp.status.as_u16();

        if let Some(mut recv_body) = rsp_recv_body {
            let mut body_reader = recv_body.body_reader();
            let copy_to_clt = LimitedCopy::new(
                &mut body_reader,
                clt_w,
                &self.ctx.server_config.limited_copy_config(),
            );
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

    async fn send_request<CR, CW, UR, UW>(
        &mut self,
        adapted_req: Option<HttpTransparentRequest>,
        req_io: &mut HttpRequestIo<CR, UW>,
        rsp_io: &mut HttpResponseIo<UR, CW>,
    ) -> ServerTaskResult<Option<UpstreamAddr>>
    where
        CR: AsyncRead + Send + Unpin,
        CW: AsyncWrite + Send + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Send + Unpin,
    {
        let mut ups_w_adaptation = HttpRequestWriterForAdaptation {
            inner: &mut req_io.ups_w,
        };
        let req = adapted_req.as_ref().unwrap_or(&self.req);
        ups_w_adaptation
            .send_request_header(req)
            .await
            .map_err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed)?;
        ups_w_adaptation
            .flush()
            .await
            .map_err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed)?;
        self.http_notes.mark_ups_send_header();

        self.do_recv_response(rsp_io).await
    }

    async fn do_recv_response<CW, UR>(
        &mut self,
        rsp_io: &mut HttpResponseIo<UR, CW>,
    ) -> ServerTaskResult<Option<UpstreamAddr>>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        match tokio::time::timeout(
            self.ctx.h1_rsp_hdr_recv_timeout(),
            HttpTransparentResponse::parse(
                &mut rsp_io.ups_r,
                &self.req.method,
                self.req.keep_alive(),
                self.ctx.h1_interception().rsp_head_max_size,
            ),
        )
        .await
        {
            Ok(Ok((rsp, head_bytes))) => self.send_response(rsp, head_bytes, rsp_io).await,
            Ok(Err(e)) => Err(e.into()),
            Err(_) => Err(ServerTaskError::UpstreamAppTimeout(
                "timeout to receive response header",
            )),
        }
    }

    async fn send_response<CW, UR>(
        &mut self,
        rsp: HttpTransparentResponse,
        rsp_head: Bytes,
        rsp_io: &mut HttpResponseIo<UR, CW>,
    ) -> ServerTaskResult<Option<UpstreamAddr>>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        self.send_error_response = false;
        self.http_notes.origin_status = rsp.code;
        self.http_notes.mark_rsp_recv_hdr();
        if !rsp.keep_alive() {
            self.should_close = true;
        }

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
        } else if let Some(body_type) = rsp.body_type(&self.req.method) {
            self.send_response_body(rsp_io, body_type).await?;
            Ok(None)
        } else {
            Ok(None)
        }
    }

    async fn send_response_body<UR, CW>(
        &mut self,
        rsp_io: &mut HttpResponseIo<UR, CW>,
        body_type: HttpBodyType,
    ) -> ServerTaskResult<()>
    where
        UR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
    {
        let mut body_reader = HttpBodyReader::new(
            &mut rsp_io.ups_r,
            body_type,
            self.ctx.h1_interception().body_line_max_len,
        );

        let mut ups_to_clt = LimitedCopy::new(
            &mut body_reader,
            &mut rsp_io.clt_w,
            &self.ctx.server_config.limited_copy_config(),
        );

        let idle_duration = self.ctx.server_config.task_idle_check_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;
        let max_idle_count = self.ctx.task_max_idle_count();

        loop {
            tokio::select! {
                biased;

                r = &mut ups_to_clt => {
                    return match r {
                        Ok(_) => {
                            // clt_w is already flushed
                            Ok(())
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => {
                            let _ = ups_to_clt.write_flush().await;
                            Err(ServerTaskError::UpstreamReadFailed(e))
                        }
                        Err(LimitedCopyError::WriteFailed(e)) => Err(ServerTaskError::ClientTcpWriteFailed(e)),
                    };
                }
                _ = idle_interval.tick() => {
                    if ups_to_clt.is_idle() {
                        idle_count += 1;
                        if idle_count >= max_idle_count {
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

                    if self.ctx.belongs_to_blocked_user() {
                        let _ = ups_to_clt.write_flush().await;
                        return Err(ServerTaskError::CanceledAsUserBlocked);
                    }

                    if self.ctx.server_force_quit() {
                        let _ = ups_to_clt.write_flush().await;
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            }
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
