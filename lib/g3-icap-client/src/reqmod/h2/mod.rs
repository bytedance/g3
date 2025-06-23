/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::{BufMut, Bytes};
use h2::client::SendRequest;
use h2::ext::Protocol;
use h2::{RecvStream, SendStream};
use http::{Extensions, Request, Response};
use tokio::time::Instant;

use g3_h2::H2StreamFromChunkedTransfer;
use g3_http::server::HttpAdaptedRequest;
use g3_io_ext::{IdleCheck, StreamCopyConfig};
use g3_types::net::HttpHeaderMap;

use super::IcapReqmodClient;
use crate::{IcapClientConnection, IcapClientReader, IcapServiceClient, IcapServiceOptions};

pub use crate::reqmod::h1::HttpAdapterErrorResponse;

mod error;
pub use error::H2ReqmodAdaptationError;

mod recv_request;
mod recv_response;

mod bidirectional;
use bidirectional::{BidirectionalRecvHttpRequest, BidirectionalRecvIcapResponse};

mod preview;

mod forward_body;
mod forward_header;

impl IcapReqmodClient {
    pub async fn h2_adapter<I: IdleCheck>(
        &self,
        copy_config: StreamCopyConfig,
        http_body_line_max_size: usize,
        http_trailer_max_size: usize,
        http_rsp_head_recv_timeout: Duration,
        http_req_add_no_via_header: bool,
        idle_checker: I,
    ) -> anyhow::Result<H2RequestAdapter<I>> {
        let icap_client = self.inner.clone();
        let (icap_connection, icap_options) = icap_client.fetch_connection().await?;
        Ok(H2RequestAdapter {
            icap_client,
            icap_connection,
            icap_options,
            copy_config,
            http_body_line_max_size,
            http_trailer_max_size,
            http_rsp_head_recv_timeout,
            http_req_add_no_via_header,
            idle_checker,
            client_addr: None,
            client_username: None,
        })
    }
}

pub struct H2RequestAdapter<I: IdleCheck> {
    icap_client: Arc<IcapServiceClient>,
    icap_connection: IcapClientConnection,
    icap_options: Arc<IcapServiceOptions>,
    copy_config: StreamCopyConfig,
    http_body_line_max_size: usize,
    http_trailer_max_size: usize,
    http_rsp_head_recv_timeout: Duration,
    http_req_add_no_via_header: bool,
    idle_checker: I,
    client_addr: Option<SocketAddr>,
    client_username: Option<Arc<str>>,
}

pub struct ReqmodAdaptationRunState {
    task_create_instant: Instant,
    pub dur_ups_send_header: Option<Duration>,
    pub dur_ups_send_all: Option<Duration>,
    pub dur_ups_recv_header: Option<Duration>,
    pub(crate) respond_shared_headers: Option<HttpHeaderMap>,
}

impl ReqmodAdaptationRunState {
    pub fn new(task_create_instant: Instant) -> Self {
        ReqmodAdaptationRunState {
            task_create_instant,
            dur_ups_send_header: None,
            dur_ups_send_all: None,
            dur_ups_recv_header: None,
            respond_shared_headers: None,
        }
    }

    pub fn take_respond_shared_headers(&mut self) -> Option<HttpHeaderMap> {
        self.respond_shared_headers.take()
    }

    pub(crate) fn mark_ups_send_header(&mut self) {
        self.dur_ups_send_header = Some(self.task_create_instant.elapsed());
    }

    pub(crate) fn mark_ups_send_no_body(&mut self) {
        self.dur_ups_send_all = self.dur_ups_send_header;
    }

    pub(crate) fn mark_ups_send_all(&mut self) {
        self.dur_ups_send_all = Some(self.task_create_instant.elapsed());
    }

    pub(crate) fn mark_ups_recv_header(&mut self) {
        self.dur_ups_recv_header = Some(self.task_create_instant.elapsed());
    }
}

impl<I: IdleCheck> H2RequestAdapter<I> {
    pub fn set_client_addr(&mut self, addr: SocketAddr) {
        self.client_addr = Some(addr);
    }

    pub fn set_client_username(&mut self, user: Arc<str>) {
        self.client_username = Some(user);
    }

    fn push_extended_headers(&self, data: &mut Vec<u8>, extensions: Option<&Extensions>) {
        data.put_slice(b"X-Transformed-From: HTTP/2.0\r\n");
        if let Some(addr) = self.client_addr {
            crate::serialize::add_client_addr(data, addr);
        }
        if let Some(user) = &self.client_username {
            crate::serialize::add_client_username(data, user);
        }
        if let Some(ext) = extensions {
            if let Some(p) = ext.get::<Protocol>() {
                data.put_slice(b"X-HTTP-Upgrade: ");
                data.put_slice(p.as_str().as_bytes());
                data.put_slice(b"\r\n");
            }
        }
    }

    fn preview_size(&self) -> Option<usize> {
        if self.icap_client.config.disable_preview {
            return None;
        }
        self.icap_options.preview_size
    }

    pub async fn xfer(
        self,
        state: &mut ReqmodAdaptationRunState,
        http_request: Request<()>,
        clt_body: RecvStream,
        ups_send_request: SendRequest<Bytes>,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        if clt_body.is_end_stream() {
            self.xfer_without_body(state, http_request, ups_send_request)
                .await
        } else if let Some(preview_size) = self.preview_size() {
            self.xfer_with_preview(
                state,
                http_request,
                clt_body,
                ups_send_request,
                preview_size,
            )
            .await
        } else {
            self.xfer_without_preview(state, http_request, clt_body, ups_send_request)
                .await
        }
    }
}

pub enum ReqmodAdaptationEndState {
    OriginalTransferred(Response<RecvStream>),
    AdaptedTransferred(HttpAdaptedRequest, Response<RecvStream>),
    HttpErrResponse(HttpAdapterErrorResponse, Option<ReqmodRecvHttpResponseBody>),
}

pub enum ReqmodAdaptationMidState {
    OriginalRequest(Request<()>),
    AdaptedRequest(HttpAdaptedRequest, Request<()>),
    HttpErrResponse(HttpAdapterErrorResponse, Option<ReqmodRecvHttpResponseBody>),
}

pub struct ReqmodRecvHttpResponseBody {
    icap_client: Arc<IcapServiceClient>,
    icap_keepalive: bool,
    icap_connection: IcapClientConnection,
    copy_config: StreamCopyConfig,
    http_body_line_max_size: usize,
    http_trailer_max_size: usize,
}

impl ReqmodRecvHttpResponseBody {
    pub fn body_transfer<'a>(
        &'a mut self,
        send_stream: &'a mut SendStream<Bytes>,
    ) -> H2StreamFromChunkedTransfer<'a, IcapClientReader> {
        H2StreamFromChunkedTransfer::new(
            &mut self.icap_connection.reader,
            send_stream,
            &self.copy_config,
            self.http_body_line_max_size,
            self.http_trailer_max_size,
        )
    }

    pub async fn save_connection(mut self) {
        self.icap_connection.mark_reader_finished();
        if self.icap_keepalive {
            self.icap_client.save_connection(self.icap_connection);
        }
    }
}
