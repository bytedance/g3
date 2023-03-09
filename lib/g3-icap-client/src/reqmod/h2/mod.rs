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

use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::{BufMut, Bytes};
use h2::client::SendRequest;
use h2::{RecvStream, SendStream};
use http::{HeaderMap, Request, Response};
use tokio::time::Instant;

use g3_h2::H2StreamFromChunkedTransfer;
use g3_http::server::HttpAdaptedRequest;
use g3_io_ext::{IdleCheck, LimitedCopyConfig};

pub use super::h1::HttpAdapterErrorResponse;
use super::IcapReqmodClient;
use crate::{IcapClientConnection, IcapServiceClient, IcapServiceOptions};

mod error;
pub use error::H2ReqmodAdaptationError;

mod recv_request;
mod recv_response;

mod bidirectional;
use crate::service::IcapClientReader;
use bidirectional::{BidirectionalRecvHttpRequest, BidirectionalRecvIcapResponse};

mod forward_body;
mod forward_header;
mod preview;

impl IcapReqmodClient {
    pub async fn h2_adapter<I: IdleCheck>(
        &self,
        copy_config: LimitedCopyConfig,
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
    copy_config: LimitedCopyConfig,
    http_body_line_max_size: usize,
    http_trailer_max_size: usize,
    http_rsp_head_recv_timeout: Duration,
    http_req_add_no_via_header: bool,
    idle_checker: I,
    client_addr: Option<SocketAddr>,
    client_username: Option<String>,
}

pub struct ReqmodAdaptationRunState {
    task_create_instant: Instant,
    pub dur_ups_send_header: Option<Duration>,
    pub dur_ups_send_all: Option<Duration>,
    pub dur_ups_recv_header: Option<Duration>,
    pub(crate) icap_io_finished: bool,
    pub(crate) respond_shared_headers: Option<HeaderMap>,
}

impl ReqmodAdaptationRunState {
    pub fn new(task_create_instant: Instant) -> Self {
        ReqmodAdaptationRunState {
            task_create_instant,
            dur_ups_send_header: None,
            dur_ups_send_all: None,
            dur_ups_recv_header: None,
            icap_io_finished: false,
            respond_shared_headers: None,
        }
    }

    pub fn take_respond_shared_headers(&mut self) -> Option<HeaderMap> {
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

    pub fn set_client_username(&mut self, user: &str) {
        self.client_username = Some(user.to_string());
    }

    fn push_extended_headers(&self, data: &mut Vec<u8>) {
        if let Some(addr) = self.client_addr {
            let _ = write!(data, "X-Client-IP: {}\r\n", addr.ip());
            let _ = write!(data, "X-Client-Port: {}\r\n", addr.port());
        }
        if let Some(user) = &self.client_username {
            data.put_slice(b"X-Client-Username: ");
            data.put_slice(user.as_bytes());
            data.put_slice(b"\r\n");
        }
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
        } else if let Some(preview_size) = self.icap_options.preview_size {
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

pub struct ReqmodRecvHttpResponseBody {
    icap_client: Arc<IcapServiceClient>,
    icap_keepalive: bool,
    icap_connection: IcapClientConnection,
    copy_config: LimitedCopyConfig,
    http_body_line_max_size: usize,
    http_trailer_max_size: usize,
    has_trailer: bool,
}

impl ReqmodRecvHttpResponseBody {
    pub fn body_transfer<'a>(
        &'a mut self,
        send_stream: &'a mut SendStream<Bytes>,
    ) -> H2StreamFromChunkedTransfer<'a, IcapClientReader> {
        H2StreamFromChunkedTransfer::new(
            &mut self.icap_connection.1,
            send_stream,
            &self.copy_config,
            self.http_body_line_max_size,
            self.http_trailer_max_size,
            self.has_trailer,
        )
    }

    pub async fn save_connection(self) {
        if self.icap_keepalive {
            self.icap_client.save_connection(self.icap_connection).await;
        }
    }
}
