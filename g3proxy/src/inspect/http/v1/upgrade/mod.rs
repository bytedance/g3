/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use anyhow::anyhow;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use http::{StatusCode, Version};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::time::Instant;

use g3_dpi::Protocol;
use g3_http::client::HttpTransparentResponse;
use g3_http::server::{HttpTransparentRequest, UriExt};
use g3_http::{HttpBodyReader, HttpBodyType};
use g3_icap_client::reqmod::IcapReqmodClient;
use g3_icap_client::reqmod::h1::{
    H1ReqmodAdaptationError, HttpAdapterErrorResponse, HttpRequestAdapter,
    ReqmodAdaptationMidState, ReqmodAdaptationRunState, ReqmodRecvHttpResponseBody,
};
use g3_io_ext::{LimitedWriteExt, OnceBufReader, StreamCopy, StreamCopyError};
use g3_slog_types::{LtDateTime, LtDuration, LtHttpUri, LtUpstreamAddr, LtUuid};
use g3_types::net::{HttpUpgradeToken, UpstreamAddr, WebSocketNotes};

use super::{H1InterceptionError, HttpRequest, HttpRequestIo, HttpResponseIo};
use crate::config::server::ServerConfig;
use crate::inspect::{BoxAsyncRead, BoxAsyncWrite, StreamInspectContext, StreamInspection};
use crate::log::inspect::InspectSource;
use crate::log::inspect::stream::StreamInspectLog;
use crate::module::http_forward::HttpProxyClientResponse;
use crate::serve::{ServerIdleChecker, ServerTaskError, ServerTaskResult};

macro_rules! intercept_log {
    ($obj:tt, $r:expr, $($args:tt)+) => {
        if let Some(logger) = $obj.ctx.intercept_logger() {
            slog::info!(logger, $($args)+;
                "intercept_type" => "HttpUpgrade",
                "task_id" => LtUuid($obj.ctx.server_task_id()),
                "depth" => $obj.ctx.inspection_depth,
                "request_id" => $obj.req_id,
                "next_protocol" => $r.as_ref().map(|v| v.0.to_string()),
                "next_upstream" => $r.as_ref().map(|v| LtUpstreamAddr(&v.1)),
                "received_at" => LtDateTime(&$obj.http_notes.receive_datetime),
                "uri" => LtHttpUri::new(&$obj.req.uri, $obj.ctx.log_uri_max_chars()),
                "rsp_status" => $obj.http_notes.rsp_status,
                "origin_status" => $obj.http_notes.origin_status,
                "dur_req_send_hdr" => LtDuration($obj.http_notes.dur_req_send_hdr),
                "dur_req_pipeline" => LtDuration($obj.http_notes.dur_req_pipeline),
                "dur_rsp_recv_hdr" => LtDuration($obj.http_notes.dur_rsp_recv_hdr),
            );
        }
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
    fn new(datetime_received: DateTime<Utc>, time_received: Instant) -> Self {
        let dur_req_pipeline = time_received.elapsed();
        HttpForwardTaskNotes {
            rsp_status: 0,
            origin_status: 0,
            receive_ins: time_received,
            receive_datetime: datetime_received,
            dur_req_send_hdr: Duration::default(),
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

pub(super) struct H1UpgradeTask<SC: ServerConfig> {
    ctx: StreamInspectContext<SC>,
    req: HttpTransparentRequest,
    req_id: usize,
    send_error_response: bool,
    should_close: bool,
    http_notes: HttpForwardTaskNotes,
    ws_notes: Option<WebSocketNotes>,
}

impl<SC> H1UpgradeTask<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(super) fn new(ctx: StreamInspectContext<SC>, req: HttpRequest, req_id: usize) -> Self {
        let http_notes = HttpForwardTaskNotes::new(req.datetime_received, req.time_received);
        H1UpgradeTask {
            ctx,
            req: req.inner,
            req_id,
            send_error_response: true,
            should_close: false,
            http_notes,
            ws_notes: None,
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

    async fn reply_fatal<CW>(&mut self, rsp: HttpProxyClientResponse, clt_w: &mut CW)
    where
        CW: AsyncWrite + Unpin,
    {
        self.should_close = true;
        if rsp.reply_err_to_request(clt_w).await.is_ok() {
            self.http_notes.rsp_status = rsp.status();
        }
    }

    async fn check_blocked<CW>(&mut self, clt_w: &mut CW) -> ServerTaskResult<()>
    where
        CW: AsyncWrite + Unpin,
    {
        match self.req.retain_upgrade_token(|req, p| {
            if matches!(p, HttpUpgradeToken::Websocket) {
                let Some(http_host) = &req.host else {
                    return false;
                };
                return !self
                    .ctx
                    .websocket_inspect_action(http_host.host())
                    .is_block();
            } else if matches!(p, HttpUpgradeToken::ConnectIp) {
                return false;
            }
            true
        }) {
            Some(0) => {
                self.reply_fatal(HttpProxyClientResponse::forbidden(self.req.version), clt_w)
                    .await;
                Err(ServerTaskError::InternalAdapterError(anyhow!(
                    "upgrade protocol blocked by inspection policy"
                )))
            }
            Some(_) => Ok(()),
            None => {
                self.reply_fatal(
                    HttpProxyClientResponse::bad_request(self.req.version),
                    clt_w,
                )
                .await;
                Err(ServerTaskError::InternalAdapterError(anyhow!(
                    "no Upgrade header found in HTTP upgrade request"
                )))
            }
        }
    }

    pub(super) async fn forward_original<CW, UR, UW>(
        &mut self,
        rsp_io: &mut HttpResponseIo<CW, UR, UW>,
    ) -> Option<(HttpUpgradeToken, UpstreamAddr)>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        match self.do_forward_original(rsp_io).await {
            Ok(v) => {
                intercept_log!(self, &v, "ok");
                v
            }
            Err(e) => {
                if self.send_error_response {
                    self.reply_task_err(&e, &mut rsp_io.clt_w).await;
                }
                intercept_log!(self, &None::<(HttpUpgradeToken, UpstreamAddr)>, "{e}");
                None
            }
        }
    }

    pub(super) async fn do_forward_original<CW, UR, UW>(
        &mut self,
        rsp_io: &mut HttpResponseIo<CW, UR, UW>,
    ) -> ServerTaskResult<Option<(HttpUpgradeToken, UpstreamAddr)>>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        self.check_blocked(&mut rsp_io.clt_w).await?;
        self.send_request(None, rsp_io).await
    }

    pub(super) async fn forward_icap<CW, UR, UW>(
        &mut self,
        rsp_io: &mut HttpResponseIo<CW, UR, UW>,
        reqmod_client: &IcapReqmodClient,
    ) -> Option<(HttpUpgradeToken, UpstreamAddr)>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        match self.do_forward_icap(rsp_io, reqmod_client).await {
            Ok(v) => {
                intercept_log!(self, &v, "ok");
                v
            }
            Err(e) => {
                if self.send_error_response {
                    self.reply_task_err(&e, &mut rsp_io.clt_w).await;
                }
                intercept_log!(self, &None::<(HttpUpgradeToken, UpstreamAddr)>, "{e}");
                None
            }
        }
    }

    async fn do_forward_icap<CW, UR, UW>(
        &mut self,
        rsp_io: &mut HttpResponseIo<CW, UR, UW>,
        reqmod_client: &IcapReqmodClient,
    ) -> ServerTaskResult<Option<(HttpUpgradeToken, UpstreamAddr)>>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        self.check_blocked(&mut rsp_io.clt_w).await?;
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
                    adapter.set_client_username(username.clone());
                }
                let mut adaptation_state =
                    ReqmodAdaptationRunState::new(self.http_notes.receive_ins);
                self.forward_with_adaptation(rsp_io, adapter, &mut adaptation_state)
                    .await
            }
            Err(e) => {
                if reqmod_client.bypass() {
                    self.send_request(None, rsp_io).await
                } else {
                    Err(ServerTaskError::InternalAdapterError(e))
                }
            }
        }
    }

    async fn forward_with_adaptation<CW, UR, UW>(
        &mut self,
        rsp_io: &mut HttpResponseIo<CW, UR, UW>,
        icap_adapter: HttpRequestAdapter<ServerIdleChecker>,
        adaptation_state: &mut ReqmodAdaptationRunState,
    ) -> ServerTaskResult<Option<(HttpUpgradeToken, UpstreamAddr)>>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        match icap_adapter.xfer_connect(adaptation_state, &self.req).await {
            Ok(ReqmodAdaptationMidState::OriginalRequest) => self.send_request(None, rsp_io).await,
            Ok(ReqmodAdaptationMidState::AdaptedRequest(final_req)) => {
                self.send_request(Some(final_req), rsp_io).await
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
            let copy_to_clt = StreamCopy::new(
                &mut body_reader,
                clt_w,
                &self.ctx.server_config.limited_copy_config(),
            );
            copy_to_clt.await.map_err(|e| match e {
                StreamCopyError::ReadFailed(e) => ServerTaskError::InternalAdapterError(anyhow!(
                    "read http error response from adapter failed: {e:?}"
                )),
                StreamCopyError::WriteFailed(e) => ServerTaskError::ClientTcpWriteFailed(e),
            })?;
            recv_body.save_connection().await;
        } else {
            clt_w
                .flush()
                .await
                .map_err(ServerTaskError::ClientTcpWriteFailed)?;
        }

        Ok(())
    }

    async fn send_request<CW, UR, UW>(
        &mut self,
        adapted_req: Option<HttpTransparentRequest>,
        rsp_io: &mut HttpResponseIo<CW, UR, UW>,
    ) -> ServerTaskResult<Option<(HttpUpgradeToken, UpstreamAddr)>>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let req = adapted_req.as_ref().unwrap_or(&self.req);
        let head_bytes = req.serialize_for_origin();
        rsp_io
            .ups_w
            .write_all_flush(&head_bytes)
            .await
            .map_err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed)?;
        self.http_notes.mark_ups_send_header();

        self.recv_response(rsp_io).await
    }

    async fn recv_response<CW, UR, UW>(
        &mut self,
        rsp_io: &mut HttpResponseIo<CW, UR, UW>,
    ) -> ServerTaskResult<Option<(HttpUpgradeToken, UpstreamAddr)>>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
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

    async fn send_response<CW, UR, UW>(
        &mut self,
        mut rsp: HttpTransparentResponse,
        rsp_head: Bytes,
        rsp_io: &mut HttpResponseIo<CW, UR, UW>,
    ) -> ServerTaskResult<Option<(HttpUpgradeToken, UpstreamAddr)>>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
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

        if rsp.code == StatusCode::SWITCHING_PROTOCOLS {
            let upgrade_protocol = match rsp.upgrade {
                Some(p) => p,
                None => {
                    self.should_close = true;
                    return Err(ServerTaskError::UpstreamAppError(anyhow!(
                        "no upgrade header found in 101 response"
                    )));
                }
            };

            match upgrade_protocol {
                HttpUpgradeToken::Websocket => {
                    let mut ws_notes = WebSocketNotes::new(self.req.uri.clone());
                    ws_notes.append_response_headers(rsp.end_to_end_headers.drain());
                    self.ws_notes = Some(ws_notes);
                }
                HttpUpgradeToken::ConnectUdp => {
                    let upstream = self
                        .req
                        .uri
                        .get_connect_udp_upstream()
                        .map_err(ServerTaskError::from)?;
                    return Ok(Some((upgrade_protocol, upstream)));
                }
                _ => {}
            }

            let upstream = self
                .req
                .host
                .take()
                .ok_or(ServerTaskError::InvalidClientProtocol(
                    "no Host header found in http upgrade request",
                ))?;
            Ok(Some((upgrade_protocol, upstream)))
        } else if let Some(body_type) = rsp.body_type(&self.req.method) {
            self.send_response_body(rsp_io, body_type).await?;
            Ok(None)
        } else {
            Ok(None)
        }
    }

    async fn send_response_body<CW, UR, UW>(
        &mut self,
        rsp_io: &mut HttpResponseIo<CW, UR, UW>,
        body_type: HttpBodyType,
    ) -> ServerTaskResult<()>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let mut body_reader = HttpBodyReader::new(
            &mut rsp_io.ups_r,
            body_type,
            self.ctx.h1_interception().body_line_max_len,
        );

        let mut ups_to_clt = StreamCopy::new(
            &mut body_reader,
            &mut rsp_io.clt_w,
            &self.ctx.server_config.limited_copy_config(),
        );

        let mut idle_interval = self.ctx.idle_wheel.register();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut ups_to_clt => {
                    return match r {
                        Ok(_) => {
                            // clt_w is already flushed
                            Ok(())
                        }
                        Err(StreamCopyError::ReadFailed(e)) => {
                            let _ = ups_to_clt.write_flush().await;
                            Err(ServerTaskError::UpstreamReadFailed(e))
                        }
                        Err(StreamCopyError::WriteFailed(e)) => Err(ServerTaskError::ClientTcpWriteFailed(e)),
                    };
                }
                n = idle_interval.tick() => {
                    if ups_to_clt.is_idle() {
                        idle_count += n;
                        if idle_count >= self.ctx.max_idle_count {
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

    pub(super) fn into_upgrade(
        mut self,
        req_io: HttpRequestIo<BoxAsyncRead>,
        rsp_io: HttpResponseIo<BoxAsyncWrite, BoxAsyncRead, BoxAsyncWrite>,
        protocol: HttpUpgradeToken,
        upstream: UpstreamAddr,
    ) -> Result<StreamInspection<SC>, H1InterceptionError> {
        let (clt_r, clt_w, ups_r, ups_w) = super::convert_io(req_io, rsp_io);

        let mut ctx = self.ctx;
        ctx.increase_inspection_depth();

        match protocol {
            HttpUpgradeToken::Http(Version::HTTP_2) => {
                StreamInspectLog::new(&ctx).log(InspectSource::HttpUpgrade, Protocol::Http2);
                let mut h2_obj = crate::inspect::http::H2InterceptObject::new(ctx, upstream);
                h2_obj.set_io(OnceBufReader::with_no_buf(clt_r), clt_w, ups_r, ups_w);
                Ok(StreamInspection::H2(h2_obj))
            }
            HttpUpgradeToken::Http(_) | HttpUpgradeToken::Tls(_, _) => {
                Err(H1InterceptionError::InvalidUpgradeProtocol(protocol))
            }
            HttpUpgradeToken::Websocket => {
                let mut ws_notes = self.ws_notes.unwrap();
                ws_notes.append_request_headers(self.req.end_to_end_headers.drain());
                StreamInspectLog::new(&ctx).log(InspectSource::HttpUpgrade, Protocol::Websocket);
                let mut websocket_obj = crate::inspect::websocket::H1WebsocketInterceptObject::new(
                    ctx, upstream, ws_notes,
                );
                websocket_obj.set_io(clt_r, clt_w, ups_r, ups_w);
                Ok(StreamInspection::Websocket(websocket_obj))
            }
            _ => {
                StreamInspectLog::new(&ctx).log(InspectSource::HttpUpgrade, Protocol::Unknown);
                let mut stream_obj =
                    crate::inspect::stream::StreamInspectObject::new(ctx, upstream);
                stream_obj.set_io(clt_r, clt_w, ups_r, ups_w);
                // Just treat it as unknown. Unknown protocol should be forbidden if needed.
                Ok(StreamInspection::StreamUnknown(stream_obj))
            }
        }
    }
}
