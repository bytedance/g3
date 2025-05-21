/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use http::Method;
use tokio::io::{AsyncBufRead, AsyncWrite};
use tokio::time::Instant;

use g3_http::HttpBodyType;
use g3_http::client::HttpAdaptedResponse;
use g3_io_ext::{IdleCheck, LimitedCopyConfig};
use g3_types::net::HttpHeaderMap;

use super::IcapRespmodClient;
use crate::reqmod::h1::HttpRequestForAdaptation;
use crate::{IcapClientConnection, IcapServiceClient, IcapServiceOptions};

mod error;
pub use error::H1RespmodAdaptationError;

mod bidirectional;
use bidirectional::{BidirectionalRecvHttpResponse, BidirectionalRecvIcapResponse};

mod recv_response;

mod forward_body;
mod forward_header;
mod preview;

mod impl_trait;

pub trait HttpResponseForAdaptation {
    fn body_type(&self, method: &Method) -> Option<HttpBodyType>;
    fn serialize_for_client(&self) -> Vec<u8>;
    fn serialize_for_adapter(&self) -> Vec<u8>;
    fn adapt_with_body(&self, other: HttpAdaptedResponse) -> Self;
    fn adapt_without_body(&self, other: HttpAdaptedResponse) -> Self;
}

#[allow(async_fn_in_trait)]
pub trait HttpResponseClientWriter<H: HttpResponseForAdaptation>: AsyncWrite {
    async fn send_response_header(&mut self, req: &H) -> io::Result<()>;
}

impl IcapRespmodClient {
    pub async fn h1_adapter<I: IdleCheck>(
        &self,
        copy_config: LimitedCopyConfig,
        http_body_line_max_size: usize,
        idle_checker: I,
    ) -> anyhow::Result<HttpResponseAdapter<I>> {
        let icap_client = self.inner.clone();
        let (icap_connection, icap_options) = icap_client.fetch_connection().await?;
        Ok(HttpResponseAdapter {
            icap_client,
            icap_connection,
            icap_options,
            copy_config,
            http_body_line_max_size,
            idle_checker,
            client_addr: None,
            client_username: None,
            respond_shared_headers: None,
        })
    }
}

pub struct HttpResponseAdapter<I: IdleCheck> {
    icap_client: Arc<IcapServiceClient>,
    icap_connection: IcapClientConnection,
    icap_options: Arc<IcapServiceOptions>,
    copy_config: LimitedCopyConfig,
    http_body_line_max_size: usize,
    idle_checker: I,
    client_addr: Option<SocketAddr>,
    client_username: Option<Arc<str>>,
    respond_shared_headers: Option<HttpHeaderMap>,
}

pub struct RespmodAdaptationRunState {
    task_create_instant: Instant,
    dur_ups_recv_header: Duration,
    pub dur_ups_recv_all: Option<Duration>,
    pub dur_clt_send_header: Option<Duration>,
    pub dur_clt_send_all: Option<Duration>,
    pub ups_read_finished: bool,
    pub clt_write_started: bool,
    pub clt_write_finished: bool,
}

impl RespmodAdaptationRunState {
    pub fn new(task_create_instant: Instant, dur_ups_recv_header: Duration) -> Self {
        RespmodAdaptationRunState {
            task_create_instant,
            dur_ups_recv_header,
            dur_ups_recv_all: None,
            dur_clt_send_header: None,
            dur_clt_send_all: None,
            ups_read_finished: false,
            clt_write_started: false,
            clt_write_finished: false,
        }
    }

    pub(crate) fn mark_ups_recv_no_body(&mut self) {
        self.dur_ups_recv_all = Some(self.dur_ups_recv_header);
        self.ups_read_finished = true;
    }

    pub(crate) fn mark_ups_recv_all(&mut self) {
        self.dur_ups_recv_all = Some(self.task_create_instant.elapsed());
        self.ups_read_finished = true;
    }

    pub(crate) fn mark_clt_send_start(&mut self) {
        self.clt_write_started = true;
    }

    pub(crate) fn mark_clt_send_header(&mut self) {
        self.dur_clt_send_header = Some(self.task_create_instant.elapsed());
    }

    pub(crate) fn mark_clt_send_no_body(&mut self) {
        self.dur_clt_send_all = self.dur_clt_send_header;
        self.clt_write_finished = true;
    }

    pub(crate) fn mark_clt_send_all(&mut self) {
        self.dur_clt_send_all = Some(self.task_create_instant.elapsed());
        self.clt_write_finished = true;
    }
}

impl<I: IdleCheck> HttpResponseAdapter<I> {
    pub fn set_client_addr(&mut self, addr: SocketAddr) {
        self.client_addr = Some(addr);
    }

    pub fn set_client_username(&mut self, user: Arc<str>) {
        self.client_username = Some(user);
    }

    pub fn set_respond_shared_headers(&mut self, shared_headers: Option<HttpHeaderMap>) {
        self.respond_shared_headers = shared_headers;
    }

    fn push_extended_headers(&self, data: &mut Vec<u8>) {
        if let Some(addr) = self.client_addr {
            crate::serialize::add_client_addr(data, addr);
        }
        if let Some(user) = &self.client_username {
            crate::serialize::add_client_username(data, user);
        }
        if let Some(map) = &self.respond_shared_headers {
            crate::serialize::add_shared(data, map);
        }
    }

    fn preview_size(&self) -> Option<usize> {
        if self.icap_client.config.disable_preview {
            return None;
        }
        self.icap_options.preview_size
    }

    pub async fn xfer<R, H, UR, CW>(
        self,
        state: &mut RespmodAdaptationRunState,
        http_request: &R,
        http_response: &H,
        ups_body_io: &mut UR,
        clt_writer: &mut CW,
    ) -> Result<RespmodAdaptationEndState<H>, H1RespmodAdaptationError>
    where
        R: HttpRequestForAdaptation,
        H: HttpResponseForAdaptation,
        UR: AsyncBufRead + Unpin,
        CW: HttpResponseClientWriter<H> + Unpin,
    {
        if let Some(body_type) = http_response.body_type(http_request.method()) {
            if let Some(preview_size) = self.preview_size() {
                self.xfer_with_preview(
                    state,
                    http_request,
                    http_response,
                    body_type,
                    ups_body_io,
                    clt_writer,
                    preview_size,
                )
                .await
            } else {
                self.xfer_without_preview(
                    state,
                    http_request,
                    http_response,
                    body_type,
                    ups_body_io,
                    clt_writer,
                )
                .await
            }
        } else {
            state.mark_ups_recv_no_body();
            self.xfer_without_body(state, http_request, http_response, clt_writer)
                .await
        }
    }
}

pub enum RespmodAdaptationEndState<H: HttpResponseForAdaptation> {
    OriginalTransferred,
    AdaptedTransferred(H),
}
