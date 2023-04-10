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
use futures_util::FutureExt;
use slog::slog_info;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::time::Instant;

use g3_daemon::log::types::{LtDateTime, LtDuration, LtUuid};
use g3_http::client::HttpTransparentResponse;
use g3_http::server::HttpTransparentRequest;
use g3_http::{HttpBodyReader, HttpBodyType};
use g3_icap_client::reqmod::h1::{
    HttpAdapterErrorResponse, HttpRequestAdapter, ReqmodAdaptationEndState,
    ReqmodAdaptationRunState, ReqmodRecvHttpResponseBody,
};
use g3_icap_client::reqmod::IcapReqmodClient;
use g3_icap_client::respmod::h1::{
    HttpResponseAdapter, RespmodAdaptationEndState, RespmodAdaptationRunState,
};
use g3_io_ext::{LimitedBufReadExt, LimitedCopy, LimitedCopyError};
use g3_types::net::HttpHeaderMap;

use super::{HttpRequest, HttpRequestIo, HttpResponseIo};
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;
use crate::log::types::{LtHttpMethod, LtHttpUri};
use crate::module::http_forward::HttpProxyClientResponse;
use crate::serve::{ServerIdleChecker, ServerTaskError, ServerTaskResult};

mod adaptation;

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        slog_info!($obj.ctx.intercept_logger(), $($args)+;
            "intercept_type" => "HttpForward",
            "task_id" => LtUuid($obj.ctx.server_task_id()),
            "depth" => $obj.ctx.inspection_depth,
            "request_id" => $obj.req_id,
            "received_at" => LtDateTime(&$obj.http_notes.receive_datetime),
            "method" => LtHttpMethod(&$obj.req.method),
            "uri" => LtHttpUri::new(&$obj.req.uri, $obj.ctx.log_uri_max_chars()),
            "rsp_status" => $obj.http_notes.rsp_status,
            "origin_status" => $obj.http_notes.origin_status,
            "dur_req_send_hdr" => LtDuration($obj.http_notes.dur_req_send_hdr),
            "dur_req_pipeline" => LtDuration($obj.http_notes.dur_req_pipeline),
            "dur_req_send_all" => LtDuration($obj.http_notes.dur_req_send_all),
            "dur_rsp_recv_hdr" => LtDuration($obj.http_notes.dur_rsp_recv_hdr),
            "dur_rsp_recv_all" => LtDuration($obj.http_notes.dur_rsp_recv_all),
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
    dur_req_send_all: Duration,
    dur_rsp_recv_hdr: Duration,
    dur_rsp_recv_all: Duration,
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
            receive_datetime: datetime_received,
            receive_ins: time_received,
            dur_req_send_hdr,
            dur_req_pipeline,
            dur_req_send_all: Duration::default(),
            dur_rsp_recv_hdr: Duration::default(),
            dur_rsp_recv_all: Duration::default(),
        }
    }

    pub(crate) fn mark_req_no_body(&mut self) {
        self.dur_req_send_all = self.dur_req_send_hdr;
    }

    pub(crate) fn mark_req_send_all(&mut self) {
        self.dur_req_send_all = self.receive_ins.elapsed();
    }

    pub(crate) fn mark_rsp_recv_hdr(&mut self) {
        self.dur_rsp_recv_hdr = self.receive_ins.elapsed();
    }

    pub(crate) fn mark_rsp_no_body(&mut self) {
        self.dur_rsp_recv_all = self.dur_rsp_recv_hdr;
    }

    pub(crate) fn mark_rsp_recv_all(&mut self) {
        self.dur_rsp_recv_all = self.receive_ins.elapsed();
    }
}

pub(super) struct H1ForwardTask<'a, SC: ServerConfig> {
    ctx: StreamInspectContext<SC>,
    req: &'a HttpTransparentRequest,
    req_id: usize,
    send_error_response: bool,
    should_close: bool,
    http_notes: HttpForwardTaskNotes,
}

impl<'a, SC: ServerConfig> H1ForwardTask<'a, SC> {
    pub(super) fn new(ctx: StreamInspectContext<SC>, req: &'a HttpRequest, req_id: usize) -> Self {
        let http_notes = HttpForwardTaskNotes::new(
            req.datetime_received,
            req.time_received,
            req.dur_req_send_hdr,
        );
        let should_close = !req.inner.keep_alive();
        H1ForwardTask {
            ctx,
            req: &req.inner,
            req_id,
            send_error_response: true,
            should_close,
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
        let body_pending = self.req.body_type().is_some();
        let rsp = HttpProxyClientResponse::from_task_err(
            e,
            self.req.version,
            self.should_close || body_pending,
        );

        if let Some(rsp) = rsp {
            if rsp.should_close() {
                self.should_close = true;
            }

            if rsp.reply_err_to_request(clt_w).await.is_err() {
                self.should_close = true;
            } else {
                self.http_notes.rsp_status = rsp.status();
            }
        } else if body_pending {
            self.should_close = true;
        }
    }

    pub(super) async fn forward_without_body<UR, CW>(&mut self, rsp_io: &mut HttpResponseIo<UR, CW>)
    where
        UR: AsyncRead + Unpin,
        CW: AsyncWrite + Send + Unpin,
    {
        self.http_notes.mark_req_no_body();
        if let Err(e) = self.do_forward_without_body(rsp_io).await {
            if self.send_error_response {
                self.reply_task_err(&e, &mut rsp_io.clt_w).await;
            }
            intercept_log!(self, "{e}");
        } else {
            intercept_log!(self, "ok");
        }
    }

    pub(super) async fn adapt_with_io<CR, CW, UR, UW>(
        &mut self,
        req_io: &mut HttpRequestIo<CR, UW>,
        rsp_io: &mut HttpResponseIo<UR, CW>,
        reqmod_client: &IcapReqmodClient,
    ) where
        CR: AsyncRead + Send + Unpin,
        CW: AsyncWrite + Send + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Send + Unpin,
    {
        let adapter = match reqmod_client
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
                if let Some(user) = self.ctx.user() {
                    adapter.set_client_username(user.name());
                }
                adapter
            }
            Err(e) => {
                if reqmod_client.bypass() {
                    self.forward_with_io(req_io, rsp_io).await;
                } else {
                    let e = ServerTaskError::InternalAdapterError(e);
                    self.reply_task_err(&e, &mut rsp_io.clt_w).await;
                    intercept_log!(self, "{e:?}");
                }
                return;
            }
        };

        let mut adaptation_state = ReqmodAdaptationRunState::new(self.http_notes.receive_ins);
        let r = self
            .run_with_adaptation(req_io, rsp_io, adapter, &mut adaptation_state)
            .await;

        if let Some(dur) = adaptation_state.dur_ups_send_header {
            self.http_notes.dur_req_send_hdr = dur;
        }
        if let Some(dur) = adaptation_state.dur_ups_send_all {
            self.http_notes.dur_req_send_all = dur;
        }
        if !adaptation_state.clt_read_finished || !adaptation_state.ups_write_finished {
            self.should_close = true;
        }

        match r {
            Ok(_) => {
                intercept_log!(self, "ok");
            }
            Err(e) => {
                if self.send_error_response {
                    self.reply_task_err(&e, &mut rsp_io.clt_w).await;
                }
                intercept_log!(self, "{e}");
            }
        }
    }

    pub(super) async fn forward_with_io<CR, CW, UR, UW>(
        &mut self,
        req_io: &mut HttpRequestIo<CR, UW>,
        rsp_io: &mut HttpResponseIo<UR, CW>,
    ) where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Send + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let r = if let Some(body_type) = self.req.body_type() {
            self.do_forward_with_body(req_io, rsp_io, body_type).await
        } else {
            self.do_forward_without_body(rsp_io).await
        };
        match r {
            Ok(_) => {
                intercept_log!(self, "ok");
            }
            Err(e) => {
                if self.send_error_response {
                    self.reply_task_err(&e, &mut rsp_io.clt_w).await;
                }
                intercept_log!(self, "{e}");
            }
        }
    }

    async fn run_with_adaptation<CR, CW, UR, UW>(
        &mut self,
        req_io: &mut HttpRequestIo<CR, UW>,
        rsp_io: &mut HttpResponseIo<UR, CW>,
        icap_adapter: HttpRequestAdapter<ServerIdleChecker>,
        adaptation_state: &mut ReqmodAdaptationRunState,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Send + Unpin,
        CW: AsyncWrite + Send + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Send + Unpin,
    {
        use adaptation::HttpRequestWriterForAdaptation;

        let mut ups_w_adaptation = HttpRequestWriterForAdaptation {
            inner: &mut req_io.ups_w,
        };
        let mut adaptation_fut = icap_adapter
            .xfer(
                adaptation_state,
                self.req,
                Some(&mut req_io.clt_r),
                &mut ups_w_adaptation,
            )
            .boxed();

        let mut rsp_head: Option<(HttpTransparentResponse, Bytes)> = None;
        loop {
            tokio::select! {
                biased;

                r = rsp_io.ups_r.fill_wait_data() => {
                    match r {
                        Ok(true) => {
                            // we got some data from upstream
                            let (rsp, bytes) = self.recv_response_header(&mut rsp_io.ups_r).await?;
                            if rsp.code == 100 { // HTTP CONTINUE
                                self.send_response_header(&mut rsp_io.clt_w, bytes).await?;
                                // continue
                            } else {
                                rsp_head = Some((rsp, bytes));
                                break;
                            }
                        }
                        Ok(false) => return Err(ServerTaskError::ClosedByUpstream),
                        Err(e) => return Err(ServerTaskError::UpstreamReadFailed(e)),
                    }
                }
                r = &mut adaptation_fut => {
                    match r {
                        Ok(ReqmodAdaptationEndState::OriginalTransferred) => {
                            break;
                        }
                        Ok(ReqmodAdaptationEndState::AdaptedTransferred(_r)) => {
                            // TODO add log for adapted request?
                            break;
                        }
                        Ok(ReqmodAdaptationEndState::HttpErrResponse(rsp, rsp_recv_body)) => {
                            return self.send_adaptation_error_response(&mut rsp_io.clt_w, rsp, rsp_recv_body).await;
                        }
                        Err(e) => return Err(e.into()),
                    }
                }
            }
        }
        drop(adaptation_fut);

        let rsp_head = match rsp_head {
            Some(header) => {
                if !adaptation_state.clt_read_finished || !adaptation_state.ups_write_finished {
                    // not all client data transferred, drop the connection
                    self.should_close = true;
                }
                header
            }
            None => {
                match tokio::time::timeout(
                    self.ctx.h1_interception().rsp_head_recv_timeout,
                    self.recv_final_response_header(rsp_io),
                )
                .await
                {
                    Ok(Ok(v)) => v,
                    Ok(Err(e)) => return Err(e),
                    Err(_) => {
                        return Err(ServerTaskError::UpstreamAppTimeout(
                            "timeout to receive response header",
                        ))
                    }
                }
            }
        };

        self.send_response(
            rsp_head.0,
            rsp_head.1,
            rsp_io,
            adaptation_state.take_respond_shared_headers(),
        )
        .await?;

        Ok(())
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

    async fn do_forward_without_body<UR, CW>(
        &mut self,
        rsp_io: &mut HttpResponseIo<UR, CW>,
    ) -> ServerTaskResult<()>
    where
        UR: AsyncRead + Unpin,
        CW: AsyncWrite + Send + Unpin,
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
            Ok(Ok((rsp, head_bytes))) => self.send_response(rsp, head_bytes, rsp_io, None).await,
            Ok(Err(e)) => Err(e.into()),
            Err(_) => Err(ServerTaskError::UpstreamAppTimeout(
                "timeout to receive response header",
            )),
        }
    }

    async fn do_forward_with_body<CR, CW, UR, UW>(
        &mut self,
        req_io: &mut HttpRequestIo<CR, UW>,
        rsp_io: &mut HttpResponseIo<UR, CW>,
        body_type: HttpBodyType,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Send + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let mut clt_body_reader = HttpBodyReader::new(
            &mut req_io.clt_r,
            body_type,
            self.ctx.h1_interception().body_line_max_len,
        );
        let mut rsp_head: Option<(HttpTransparentResponse, Bytes)> = None;

        let mut clt_to_ups = LimitedCopy::new(
            &mut clt_body_reader,
            &mut req_io.ups_w,
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

                r = rsp_io.ups_r.fill_wait_data() => {
                    match r {
                        Ok(true) => {
                            // we got some data from upstream
                            let (rsp, bytes) = self.recv_response_header(&mut rsp_io.ups_r).await?;
                            if rsp.code == 100 { // HTTP CONTINUE
                                self.send_response_header(&mut rsp_io.clt_w, bytes).await?;
                                // continue
                            } else {
                                rsp_head = Some((rsp, bytes));
                                break;
                            }
                        }
                        Ok(false) => return Err(ServerTaskError::ClosedByUpstream),
                        Err(e) => return Err(ServerTaskError::UpstreamReadFailed(e)),
                    }
                }
                r = &mut clt_to_ups => {
                    r.map_err(|e| match e {
                        LimitedCopyError::ReadFailed(e) => ServerTaskError::ClientTcpReadFailed(e),
                        LimitedCopyError::WriteFailed(e) => ServerTaskError::UpstreamWriteFailed(e),
                    })?;
                    self.http_notes.mark_req_send_all();
                    break;
                }
                _ = idle_interval.tick() => {
                    if clt_to_ups.is_idle() {
                        idle_count += 1;
                        if idle_count >= max_idle_count {
                            return if clt_to_ups.no_cached_data() {
                                Err(ServerTaskError::ClientAppTimeout("idle while reading request body"))
                            } else {
                                Err(ServerTaskError::UpstreamAppTimeout("idle while sending request body"))
                            };
                        }
                    } else {
                        idle_count = 0;
                        clt_to_ups.reset_active();
                    }

                    if self.ctx.belongs_to_blocked_user() {
                        return Err(ServerTaskError::CanceledAsUserBlocked);
                    }

                    if self.ctx.server_force_quit() {
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            }
        }

        let copy_done = clt_to_ups.finished();
        let rsp_head = match rsp_head {
            Some(header) => {
                if !clt_body_reader.finished() {
                    // not all client data read in, drop the client connection
                    self.should_close = true;
                }
                if !copy_done {
                    // not all client data sent out, drop the remote connection
                    self.should_close = true;
                }
                header
            }
            None => {
                match tokio::time::timeout(
                    self.ctx.h1_interception().rsp_head_recv_timeout,
                    self.recv_final_response_header(rsp_io),
                )
                .await
                {
                    Ok(Ok(v)) => v,
                    Ok(Err(e)) => return Err(e),
                    Err(_) => {
                        return Err(ServerTaskError::UpstreamAppTimeout(
                            "timeout to receive response header",
                        ))
                    }
                }
            }
        };

        self.send_response(rsp_head.0, rsp_head.1, rsp_io, None)
            .await?;

        Ok(())
    }

    async fn recv_response_header<UR>(
        &mut self,
        ups_r: &mut UR,
    ) -> ServerTaskResult<(HttpTransparentResponse, Bytes)>
    where
        UR: AsyncBufRead + Unpin,
    {
        HttpTransparentResponse::parse(
            ups_r,
            &self.req.method,
            self.req.keep_alive(),
            self.ctx.h1_interception().rsp_head_max_size,
        )
        .await
        .map_err(|e| e.into())
    }

    async fn recv_final_response_header<UR, CW>(
        &mut self,
        rsp_io: &mut HttpResponseIo<UR, CW>,
    ) -> ServerTaskResult<(HttpTransparentResponse, Bytes)>
    where
        UR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
    {
        let (rsp, bytes) = self.recv_response_header(&mut rsp_io.ups_r).await?;
        if rsp.code == 100 {
            // HTTP CONTINUE
            self.send_response_header(&mut rsp_io.clt_w, bytes).await?;
            // recv the final response header
            self.recv_response_header(&mut rsp_io.ups_r).await
        } else {
            Ok((rsp, bytes))
        }
    }

    async fn send_response<UR, CW>(
        &mut self,
        mut rsp: HttpTransparentResponse,
        rsp_head: Bytes,
        rsp_io: &mut HttpResponseIo<UR, CW>,
        adaptation_respond_shared_headers: Option<HttpHeaderMap>,
    ) -> ServerTaskResult<()>
    where
        UR: AsyncRead + Unpin,
        CW: AsyncWrite + Send + Unpin,
    {
        if self.should_close {
            rsp.set_no_keep_alive();
        }
        if !rsp.keep_alive() {
            self.should_close = true;
        }
        self.http_notes.origin_status = rsp.code;
        self.http_notes.rsp_status = 0;
        self.http_notes.mark_rsp_recv_hdr();

        if let Some(respmod) = self.ctx.audit_handle.icap_respmod_client() {
            match respmod
                .h1_adapter(
                    self.ctx.server_config.limited_copy_config(),
                    self.ctx.h1_interception().body_line_max_len,
                    self.ctx.idle_checker(),
                )
                .await
            {
                Ok(mut adapter) => {
                    let mut adaptation_state = RespmodAdaptationRunState::new(
                        self.http_notes.receive_ins,
                        self.http_notes.dur_rsp_recv_hdr,
                    );
                    adapter.set_client_addr(self.ctx.task_notes.client_addr);
                    if let Some(user) = self.ctx.user() {
                        adapter.set_client_username(user.name());
                    }
                    adapter.set_respond_shared_headers(adaptation_respond_shared_headers);
                    let r = self
                        .send_response_with_adaptation(rsp, rsp_io, adapter, &mut adaptation_state)
                        .await;
                    if !adaptation_state.clt_write_finished || !adaptation_state.ups_read_finished {
                        self.should_close = true;
                    }
                    if let Some(dur) = adaptation_state.dur_ups_recv_all {
                        self.http_notes.dur_rsp_recv_all = dur;
                    }
                    self.send_error_response = !adaptation_state.clt_write_started;
                    return r;
                }
                Err(e) => {
                    if !respmod.bypass() {
                        return Err(ServerTaskError::InternalAdapterError(e));
                    }
                }
            }
        }

        self.send_response_without_adaptation(rsp, rsp_head, rsp_io)
            .await
    }

    async fn send_response_with_adaptation<UR, CW>(
        &mut self,
        rsp: HttpTransparentResponse,
        rsp_io: &mut HttpResponseIo<UR, CW>,
        icap_adapter: HttpResponseAdapter<ServerIdleChecker>,
        adaptation_state: &mut RespmodAdaptationRunState,
    ) -> ServerTaskResult<()>
    where
        UR: AsyncRead + Unpin,
        CW: AsyncWrite + Send + Unpin,
    {
        match icap_adapter
            .xfer(
                adaptation_state,
                self.req,
                &rsp,
                &mut rsp_io.ups_r,
                &mut rsp_io.clt_w,
            )
            .await
        {
            Ok(RespmodAdaptationEndState::OriginalTransferred) => {
                self.http_notes.rsp_status = rsp.code;
                Ok(())
            }
            Ok(RespmodAdaptationEndState::AdaptedTransferred(adapted_rsp)) => {
                self.http_notes.rsp_status = adapted_rsp.code;
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn send_response_without_adaptation<UR, CW>(
        &mut self,
        rsp: HttpTransparentResponse,
        rsp_head: Bytes,
        rsp_io: &mut HttpResponseIo<UR, CW>,
    ) -> ServerTaskResult<()>
    where
        UR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
    {
        self.send_error_response = false;

        if let Some(body_type) = rsp.body_type(&self.req.method) {
            self.http_notes.rsp_status = self.http_notes.origin_status; // the following function must send rsp header out
            self.send_response_body(
                rsp_head.into(),
                &mut rsp_io.ups_r,
                &mut rsp_io.clt_w,
                body_type,
            )
            .await
        } else {
            self.send_response_header(&mut rsp_io.clt_w, rsp_head)
                .await?;
            self.http_notes.rsp_status = self.http_notes.origin_status;
            self.http_notes.mark_rsp_no_body();
            Ok(())
        }
    }

    async fn send_response_header<CW>(
        &mut self,
        clt_w: &mut CW,
        head_bytes: Bytes,
    ) -> ServerTaskResult<()>
    where
        CW: AsyncWrite + Unpin,
    {
        clt_w
            .write_all(&head_bytes)
            .await
            .map_err(ServerTaskError::ClientTcpWriteFailed)?;
        clt_w
            .flush()
            .await
            .map_err(ServerTaskError::ClientTcpWriteFailed)
    }

    async fn send_response_body<UR, CW>(
        &mut self,
        header: Vec<u8>,
        ups_r: &mut UR,
        clt_w: &mut CW,
        body_type: HttpBodyType,
    ) -> ServerTaskResult<()>
    where
        UR: AsyncBufRead + Unpin,
        CW: AsyncWrite + Unpin,
    {
        let header_len = header.len() as u64;
        let mut body_reader = HttpBodyReader::new(
            ups_r,
            body_type,
            self.ctx.h1_interception().body_line_max_len,
        );

        let mut ups_to_clt = LimitedCopy::with_data(
            &mut body_reader,
            clt_w,
            &self.ctx.server_config.limited_copy_config(),
            header,
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
                            self.http_notes.mark_rsp_recv_all();
                            // clt_w is already flushed
                            Ok(())
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => {
                            if ups_to_clt.copied_size() < header_len {
                                let _ = ups_to_clt.write_flush().await; // flush rsp header to client
                            }
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
                        if ups_to_clt.copied_size() < header_len {
                            let _ = ups_to_clt.write_flush().await; // flush rsp header to client
                        }
                        return Err(ServerTaskError::CanceledAsUserBlocked);
                    }

                    if self.ctx.server_force_quit() {
                        if ups_to_clt.copied_size() < header_len {
                            let _ = ups_to_clt.write_flush().await; // flush rsp header to client
                        }
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            }
        }
    }
}
