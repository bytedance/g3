/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use bytes::Bytes;
use h2::client::SendRequest;
use h2::server::SendResponse;
use h2::{RecvStream, StreamId};
use http::{Request, Response, StatusCode, Version, header};

use g3_dpi::Protocol;
use g3_h2::{H2StreamReader, H2StreamWriter};
use g3_http::server::UriExt;
use g3_slog_types::{LtDateTime, LtDuration, LtH2StreamId, LtUpstreamAddr, LtUuid};
use g3_types::net::{HttpUpgradeToken, UpstreamAddr, WebSocketNotes};

use super::{ExchangeHead, H2StreamTransferError, HttpConnectTaskNotes};
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;
use crate::log::inspect::{InspectSource, stream::StreamInspectLog};

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        if let Some(logger) = $obj.ctx.intercept_logger() {
            slog::info!(logger, $($args)+;
                "intercept_type" => "H2ExtendedConnect",
                "task_id" => LtUuid($obj.ctx.server_task_id()),
                "depth" => $obj.ctx.inspection_depth,
                "clt_stream" => LtH2StreamId(&$obj.clt_stream_id),
                "ups_stream" => $obj.ups_stream_id.as_ref().map(LtH2StreamId),
                "next_protocol" => $obj.protocol.to_string(),
                "next_upstream" => $obj.upstream.as_ref().map(LtUpstreamAddr),
                "started_at" => LtDateTime(&$obj.http_notes.started_datetime),
                "ready_time" => LtDuration($obj.http_notes.ready_time),
                "rsp_status" => $obj.http_notes.rsp_status,
                "origin_status" => $obj.http_notes.origin_status,
                "dur_req_send_hdr" => LtDuration($obj.http_notes.dur_req_send_hdr),
                "dur_rsp_recv_hdr" => LtDuration($obj.http_notes.dur_rsp_recv_hdr),
            );
        }
    };
}

pub(crate) struct H2ExtendedConnectTask<SC: ServerConfig> {
    ctx: StreamInspectContext<SC>,
    clt_stream_id: StreamId,
    ups_stream_id: Option<StreamId>,
    protocol: HttpUpgradeToken,
    upstream: Option<UpstreamAddr>,
    http_notes: HttpConnectTaskNotes,
}

fn get_host(clt_req: &Request<RecvStream>) -> Result<Option<UpstreamAddr>, H2StreamTransferError> {
    match clt_req.headers().get(header::HOST) {
        Some(value) => {
            let host = std::str::from_utf8(value.as_bytes())
                .map_err(|_| H2StreamTransferError::InvalidHostHeader)?;
            let host = UpstreamAddr::from_str(host)
                .map_err(|_| H2StreamTransferError::InvalidHostHeader)?;
            // we didn't set the default port here, as we didn't know the server port
            Ok(Some(host))
        }
        None => Ok(None),
    }
}

impl<SC> H2ExtendedConnectTask<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(crate) fn new(
        ctx: StreamInspectContext<SC>,
        clt_stream_id: StreamId,
        protocol: HttpUpgradeToken,
    ) -> Self {
        H2ExtendedConnectTask {
            ctx,
            clt_stream_id,
            ups_stream_id: None,
            protocol,
            upstream: None,
            http_notes: HttpConnectTaskNotes::default(),
        }
    }

    fn reply_bad_request(&mut self, mut clt_send_rsp: SendResponse<Bytes>) {
        if let Ok(rsp) = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .version(Version::HTTP_2)
            .body(())
        {
            let rsp_status = rsp.status().as_u16();
            if clt_send_rsp.send_response(rsp, true).is_ok() {
                self.http_notes.rsp_status = rsp_status;
            }
        }
    }

    fn reply_forbidden(&mut self, mut clt_send_rsp: SendResponse<Bytes>) {
        if let Ok(rsp) = Response::builder()
            .status(StatusCode::FORBIDDEN)
            .version(Version::HTTP_2)
            .body(())
        {
            let rsp_status = rsp.status().as_u16();
            if clt_send_rsp.send_response(rsp, true).is_ok() {
                self.http_notes.rsp_status = rsp_status;
            }
        }
    }

    pub(crate) async fn into_running(
        mut self,
        clt_req: Request<RecvStream>,
        clt_send_rsp: SendResponse<Bytes>,
        h2s: SendRequest<Bytes>,
    ) {
        match self.protocol {
            HttpUpgradeToken::Http(_) => {
                self.cancel_and_log(clt_send_rsp, "http upgrade is not supported");
            }
            HttpUpgradeToken::Tls(_, _) => {
                self.cancel_and_log(clt_send_rsp, "tls upgrade is not supported");
            }
            HttpUpgradeToken::Websocket => {
                self.run_extended_websocket(clt_req, clt_send_rsp, h2s)
                    .await;
            }
            HttpUpgradeToken::ConnectUdp => {
                self.run_extended_connect_udp(clt_req, clt_send_rsp, h2s)
                    .await
            }
            HttpUpgradeToken::ConnectIp => {
                self.cancel_and_log(clt_send_rsp, "connect-ip upgrade is not supported");
            }
            _ => self.run_extended_unknown(clt_req, clt_send_rsp, h2s).await,
        }
    }

    fn cancel_and_log(&mut self, clt_send_rsp: SendResponse<Bytes>, reason: &str) {
        self.reply_bad_request(clt_send_rsp);
        intercept_log!(self, "{reason}");
    }

    async fn run_extended_websocket(
        mut self,
        clt_req: Request<RecvStream>,
        clt_send_rsp: SendResponse<Bytes>,
        h2s: SendRequest<Bytes>,
    ) {
        let upstream = match get_host(&clt_req) {
            Ok(Some(d)) => {
                self.upstream = Some(d.clone());
                d
            }
            Ok(None) => {
                self.reply_bad_request(clt_send_rsp);
                intercept_log!(self, "no Host header found in websocket request");
                return;
            }
            Err(e) => {
                self.reply_bad_request(clt_send_rsp);
                intercept_log!(self, "invalid request: {e}");
                return;
            }
        };

        if self
            .ctx
            .websocket_inspect_action(upstream.host())
            .is_block()
        {
            self.reply_forbidden(clt_send_rsp);
            intercept_log!(self, "websocket blocked by inspection policy");
            return;
        }

        let mut ws_notes = WebSocketNotes::new(clt_req.uri().clone());
        for (name, value) in clt_req.headers() {
            ws_notes.append_request_header(name, value);
        }
        let mut exchange_head =
            ExchangeHead::new_websocket(&self.ctx, &mut self.http_notes, &mut ws_notes);
        let exchange_head_result = exchange_head.run(clt_req, clt_send_rsp, h2s).await;
        self.ups_stream_id = exchange_head.ups_stream_id.take();
        match exchange_head_result {
            Ok(Some((clt_r, clt_w, ups_r, ups_w))) => {
                intercept_log!(self, "ok");

                self.ctx.increase_inspection_depth();
                StreamInspectLog::new(&self.ctx)
                    .log(InspectSource::H2ExtendedConnect, Protocol::Websocket);
                let websocket_obj = crate::inspect::websocket::H2WebsocketInterceptObject::new(
                    self.ctx, upstream, ws_notes,
                );
                websocket_obj.intercept(clt_r, clt_w, ups_r, ups_w).await;
            }
            Ok(None) => {
                intercept_log!(self, "finished without data");
            }
            Err(e) => {
                intercept_log!(self, "head transfer error: {e}");
            }
        }
    }

    async fn run_extended_connect_udp(
        mut self,
        clt_req: Request<RecvStream>,
        clt_send_rsp: SendResponse<Bytes>,
        h2s: SendRequest<Bytes>,
    ) {
        match clt_req.uri().get_connect_udp_upstream() {
            Ok(d) => self.upstream = Some(d),
            Err(e) => {
                self.reply_bad_request(clt_send_rsp);
                intercept_log!(self, "invalid upstream addr for connect-udp request: {e}");
                return;
            }
        }

        self.run_extended_unknown(clt_req, clt_send_rsp, h2s).await
    }

    async fn run_extended_unknown(
        mut self,
        clt_req: Request<RecvStream>,
        clt_send_rsp: SendResponse<Bytes>,
        h2s: SendRequest<Bytes>,
    ) {
        let mut exchange_head = ExchangeHead::new(&self.ctx, &mut self.http_notes);
        let exchange_head_result = exchange_head.run(clt_req, clt_send_rsp, h2s).await;
        self.ups_stream_id = exchange_head.ups_stream_id.take();
        match exchange_head_result {
            Ok(Some((clt_r, clt_w, ups_r, ups_w))) => {
                intercept_log!(self, "ok");

                self.ctx.increase_inspection_depth();
                StreamInspectLog::new(&self.ctx)
                    .log(InspectSource::H2ExtendedConnect, Protocol::Unknown);

                // do transparent for other protocols ?
                let clt_r = H2StreamReader::new(clt_r);
                let clt_w = H2StreamWriter::new(clt_w);
                let ups_r = H2StreamReader::new(ups_r);
                let ups_w = H2StreamWriter::new(ups_w);

                // Just treat it as unknown. Unknown protocol should be forbidden if needed.
                if let Err(e) = self
                    .ctx
                    .transit_inspect_unknown(clt_r, clt_w, ups_r, ups_w)
                    .await
                {
                    intercept_log!(self, "stream transfer error: {e}");
                } else {
                    intercept_log!(self, "finished");
                }
            }
            Ok(None) => {
                intercept_log!(self, "finished without data");
            }
            Err(e) => {
                intercept_log!(self, "head transfer error: {e}");
            }
        }
    }
}
