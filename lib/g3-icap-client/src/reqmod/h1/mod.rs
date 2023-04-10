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

use std::io::{self, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::BufMut;
use http::Method;
use tokio::io::{AsyncBufRead, AsyncWrite};
use tokio::time::Instant;

use g3_http::server::HttpAdaptedRequest;
use g3_http::{HttpBodyReader, HttpBodyType};
use g3_io_ext::{IdleCheck, LimitedCopyConfig};
use g3_types::net::HttpHeaderMap;

use super::IcapReqmodClient;
use crate::{IcapClientConnection, IcapServiceClient, IcapServiceOptions};

mod error;
pub use error::H1ReqmodAdaptationError;

mod bidirectional;
use bidirectional::{BidirectionalRecvHttpRequest, BidirectionalRecvIcapResponse};

mod recv_request;
mod recv_response;

mod http_response;
pub use http_response::HttpAdapterErrorResponse;

mod forward_body;
mod forward_header;
mod preview;

mod impl_trait;

pub trait HttpRequestForAdaptation {
    fn method(&self) -> &Method;
    fn body_type(&self) -> Option<HttpBodyType>;
    fn serialize_for_adapter(&self) -> Vec<u8>;
    fn append_trailer_header(&self, buf: &mut Vec<u8>);
    fn adapt_to(&self, other: HttpAdaptedRequest) -> Self;
}

#[async_trait]
pub trait HttpRequestUpstreamWriter<H: HttpRequestForAdaptation>: AsyncWrite {
    async fn send_request_header(&mut self, req: &H) -> io::Result<()>;
}

impl IcapReqmodClient {
    pub async fn h1_adapter<I: IdleCheck>(
        &self,
        copy_config: LimitedCopyConfig,
        http_body_line_max_size: usize,
        http_req_add_no_via_header: bool,
        idle_checker: I,
    ) -> anyhow::Result<HttpRequestAdapter<I>> {
        let icap_client = self.inner.clone();
        let (icap_connection, icap_options) = icap_client.fetch_connection().await?;
        Ok(HttpRequestAdapter {
            icap_client,
            icap_connection,
            icap_options,
            copy_config,
            http_body_line_max_size,
            http_req_add_no_via_header,
            idle_checker,
            client_addr: None,
            client_username: None,
        })
    }
}

pub struct HttpRequestAdapter<I: IdleCheck> {
    icap_client: Arc<IcapServiceClient>,
    icap_connection: IcapClientConnection,
    icap_options: Arc<IcapServiceOptions>,
    copy_config: LimitedCopyConfig,
    http_body_line_max_size: usize,
    http_req_add_no_via_header: bool,
    idle_checker: I,
    client_addr: Option<SocketAddr>,
    client_username: Option<String>,
}

pub struct ReqmodAdaptationRunState {
    task_create_instant: Instant,
    pub dur_ups_send_header: Option<Duration>,
    pub dur_ups_send_all: Option<Duration>,
    pub clt_read_finished: bool,
    pub ups_write_finished: bool,
    pub(crate) icap_io_finished: bool,
    pub(crate) respond_shared_headers: Option<HttpHeaderMap>,
}

impl ReqmodAdaptationRunState {
    pub fn new(task_create_instant: Instant) -> Self {
        ReqmodAdaptationRunState {
            task_create_instant,
            dur_ups_send_header: None,
            dur_ups_send_all: None,
            clt_read_finished: false,
            ups_write_finished: false,
            icap_io_finished: false,
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
        self.ups_write_finished = true;
    }

    pub(crate) fn mark_ups_send_all(&mut self) {
        self.dur_ups_send_all = Some(self.task_create_instant.elapsed());
        self.ups_write_finished = true;
    }
}

impl<I: IdleCheck> HttpRequestAdapter<I> {
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

    pub async fn xfer<H, CR, UW>(
        self,
        state: &mut ReqmodAdaptationRunState,
        http_request: &H,
        clt_body_io: Option<&mut CR>,
        ups_writer: &mut UW,
    ) -> Result<ReqmodAdaptationEndState<H>, H1ReqmodAdaptationError>
    where
        H: HttpRequestForAdaptation,
        CR: AsyncBufRead + Unpin,
        UW: HttpRequestUpstreamWriter<H> + Unpin,
    {
        if let Some(body_type) = http_request.body_type() {
            let Some(clt_body_io) = clt_body_io else {
                return Err(H1ReqmodAdaptationError::InternalServerError("no client http body io supplied while body type is not none"));
            };
            if let Some(preview_size) = self.icap_options.preview_size {
                self.xfer_with_preview(
                    state,
                    http_request,
                    body_type,
                    clt_body_io,
                    ups_writer,
                    preview_size,
                )
                .await
            } else {
                self.xfer_without_preview(state, http_request, body_type, clt_body_io, ups_writer)
                    .await
            }
        } else {
            state.clt_read_finished = true;
            self.xfer_without_body(state, http_request, ups_writer)
                .await
        }
    }
}

pub enum ReqmodAdaptationEndState<H: HttpRequestForAdaptation> {
    OriginalTransferred,
    AdaptedTransferred(H),
    HttpErrResponse(HttpAdapterErrorResponse, Option<ReqmodRecvHttpResponseBody>),
}

pub struct ReqmodRecvHttpResponseBody {
    icap_client: Arc<IcapServiceClient>,
    icap_keepalive: bool,
    icap_connection: IcapClientConnection,
    has_trailer: bool,
}

impl ReqmodRecvHttpResponseBody {
    pub fn body_reader(&mut self) -> HttpBodyReader<'_, impl AsyncBufRead> {
        let body_type = if self.has_trailer {
            HttpBodyType::ChunkedWithTrailer
        } else {
            HttpBodyType::ChunkedWithoutTrailer
        };
        HttpBodyReader::new(&mut self.icap_connection.1, body_type, 1024)
    }

    pub async fn save_connection(self) {
        if self.icap_keepalive {
            self.icap_client.save_connection(self.icap_connection).await;
        }
    }
}
