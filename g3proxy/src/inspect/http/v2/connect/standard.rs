/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use bytes::Bytes;
use h2::client::SendRequest;
use h2::server::SendResponse;
use h2::{RecvStream, SendStream, StreamId};
use http::{Request, Response, StatusCode, Version};

use g3_h2::{H2StreamReader, H2StreamWriter};
use g3_http::server::UriExt;
use g3_slog_types::{LtDateTime, LtDuration, LtH2StreamId, LtUpstreamAddr, LtUuid};
use g3_types::net::UpstreamAddr;

use super::{ExchangeHead, HttpConnectTaskNotes};
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        if let Some(logger) = $obj.ctx.intercept_logger() {
            slog::info!(logger, $($args)+;
                "intercept_type" => "H2Connect",
                "task_id" => LtUuid($obj.ctx.server_task_id()),
                "depth" => $obj.ctx.inspection_depth,
                "clt_stream" => LtH2StreamId(&$obj.clt_stream_id),
                "ups_stream" => $obj.ups_stream_id.as_ref().map(LtH2StreamId),
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

pub(crate) struct H2ConnectTask<SC: ServerConfig> {
    ctx: StreamInspectContext<SC>,
    clt_stream_id: StreamId,
    ups_stream_id: Option<StreamId>,
    upstream: Option<UpstreamAddr>,
    http_notes: HttpConnectTaskNotes,
}

impl<SC> H2ConnectTask<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(crate) fn new(ctx: StreamInspectContext<SC>, clt_stream_id: StreamId) -> Self {
        H2ConnectTask {
            ctx,
            clt_stream_id,
            ups_stream_id: None,
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

    pub(crate) async fn into_running(
        mut self,
        clt_req: Request<RecvStream>,
        clt_send_rsp: SendResponse<Bytes>,
        h2s: SendRequest<Bytes>,
    ) {
        let upstream = match clt_req.uri().get_upstream_with_default_port(443) {
            Ok(d) => {
                self.upstream = Some(d.clone());
                d
            }
            Err(e) => {
                self.reply_bad_request(clt_send_rsp);
                intercept_log!(self, "invalid connect request: {e}");
                return;
            }
        };

        let mut exchange_head = ExchangeHead::new(&self.ctx, &mut self.http_notes);
        let exchange_head_result = exchange_head.run(clt_req, clt_send_rsp, h2s).await;
        self.ups_stream_id = exchange_head.ups_stream_id.take();
        match exchange_head_result {
            Ok(Some((clt_r, clt_w, ups_r, ups_w))) => {
                intercept_log!(self, "started");

                self.run_standard_transfer(upstream, clt_r, clt_w, ups_r, ups_w)
                    .await;
            }
            Ok(None) => {
                intercept_log!(self, "finished without data");
            }
            Err(e) => {
                intercept_log!(self, "head exchange error: {e}");
            }
        }
    }

    async fn run_standard_transfer(
        self,
        upstream: UpstreamAddr,
        clt_r: RecvStream,
        clt_w: SendStream<Bytes>,
        ups_r: RecvStream,
        ups_w: SendStream<Bytes>,
    ) {
        let clt_r = H2StreamReader::new(clt_r);
        let clt_w = H2StreamWriter::new(clt_w);
        let ups_r = H2StreamReader::new(ups_r);
        let ups_w = H2StreamWriter::new(ups_w);

        if let Err(e) = crate::inspect::stream::transit_with_inspection(
            clt_r,
            clt_w,
            ups_r,
            ups_w,
            self.ctx.clone(),
            upstream,
            None,
        )
        .await
        {
            intercept_log!(self, "data transfer error: {e}");
        } else {
            intercept_log!(self, "finished");
        }
    }
}
