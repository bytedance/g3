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
use h2::{RecvStream, SendStream};
use http::{HeaderMap, Request, Response};
use tokio::time::Instant;

use g3_http::client::HttpAdaptedResponse;
use g3_io_ext::{IdleCheck, LimitedCopyConfig};

use super::IcapRespmodClient;
use crate::{IcapClientConnection, IcapServiceClient, IcapServiceOptions};

mod error;
pub use error::H2RespmodAdaptationError;

mod bidirectional;
use bidirectional::{BidirectionalRecvHttpResponse, BidirectionalRecvIcapResponse};

mod recv_response;

mod forward_body;
mod forward_header;
mod preview;

mod impl_trait;

pub trait H2SendResponseToClient {
    fn send_response(
        &mut self,
        response: Response<()>,
        end_of_stream: bool,
    ) -> Result<SendStream<Bytes>, h2::Error>;
}

impl IcapRespmodClient {
    pub async fn h2_adapter<I: IdleCheck>(
        &self,
        copy_config: LimitedCopyConfig,
        http_body_line_max_size: usize,
        http_trailer_max_size: usize,
        idle_checker: I,
    ) -> anyhow::Result<H2ResponseAdapter<I>> {
        let icap_client = self.inner.clone();
        let (icap_connection, icap_options) = icap_client.fetch_connection().await?;
        Ok(H2ResponseAdapter {
            icap_client,
            icap_connection,
            icap_options,
            copy_config,
            http_body_line_max_size,
            http_trailer_max_size,
            idle_checker,
            client_addr: None,
            client_username: None,
            respond_shared_headers: None,
        })
    }
}

pub struct H2ResponseAdapter<I: IdleCheck> {
    icap_client: Arc<IcapServiceClient>,
    icap_connection: IcapClientConnection,
    icap_options: Arc<IcapServiceOptions>,
    copy_config: LimitedCopyConfig,
    http_body_line_max_size: usize,
    http_trailer_max_size: usize,
    idle_checker: I,
    client_addr: Option<SocketAddr>,
    client_username: Option<String>,
    respond_shared_headers: Option<HeaderMap>,
}

pub struct RespmodAdaptationRunState {
    task_create_instant: Instant,
    dur_ups_recv_header: Duration,
    pub dur_ups_recv_all: Option<Duration>,
    pub dur_clt_send_header: Option<Duration>,
    pub dur_clt_send_all: Option<Duration>,
    pub clt_write_started: bool,
    pub(crate) icap_io_finished: bool,
}

impl RespmodAdaptationRunState {
    pub fn new(task_create_instant: Instant, dur_ups_recv_header: Duration) -> Self {
        RespmodAdaptationRunState {
            task_create_instant,
            dur_ups_recv_header,
            dur_ups_recv_all: None,
            dur_clt_send_header: None,
            dur_clt_send_all: None,
            clt_write_started: false,
            icap_io_finished: false,
        }
    }

    pub(crate) fn mark_ups_recv_no_body(&mut self) {
        self.dur_ups_recv_all = Some(self.dur_ups_recv_header);
    }

    pub(crate) fn mark_ups_recv_all(&mut self) {
        self.dur_ups_recv_all = Some(self.task_create_instant.elapsed());
    }

    pub(crate) fn mark_clt_send_start(&mut self) {
        self.clt_write_started = true;
    }

    pub(crate) fn mark_clt_send_header(&mut self) {
        self.dur_clt_send_header = Some(self.task_create_instant.elapsed());
    }

    pub(crate) fn mark_clt_send_no_body(&mut self) {
        self.dur_clt_send_all = self.dur_clt_send_header;
    }

    pub(crate) fn mark_clt_send_all(&mut self) {
        self.dur_clt_send_all = Some(self.task_create_instant.elapsed());
    }
}

impl<I: IdleCheck> H2ResponseAdapter<I> {
    pub fn set_client_addr(&mut self, addr: SocketAddr) {
        self.client_addr = Some(addr);
    }

    pub fn set_client_username(&mut self, user: &str) {
        self.client_username = Some(user.to_string());
    }

    pub fn set_respond_shared_headers(&mut self, shared_headers: Option<HeaderMap>) {
        self.respond_shared_headers = shared_headers;
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
        if let Some(map) = &self.respond_shared_headers {
            for (name, value) in map {
                data.put_slice(name.as_str().as_bytes());
                data.put_slice(b": ");
                data.put_slice(value.as_bytes());
                data.put_slice(b"\r\n");
            }
        }
    }

    pub async fn xfer<CW>(
        self,
        state: &mut RespmodAdaptationRunState,
        http_request: &Request<()>,
        http_response: Response<()>,
        ups_body: RecvStream,
        clt_send_response: &mut CW,
    ) -> Result<RespmodAdaptationEndState, H2RespmodAdaptationError>
    where
        CW: H2SendResponseToClient,
    {
        if ups_body.is_end_stream() {
            state.mark_ups_recv_no_body();
            self.xfer_without_body(state, http_request, http_response, clt_send_response)
                .await
        } else if let Some(preview_size) = self.icap_options.preview_size {
            self.xfer_with_preview(
                state,
                http_request,
                http_response,
                ups_body,
                clt_send_response,
                preview_size,
            )
            .await
        } else {
            self.xfer_without_preview(
                state,
                http_request,
                http_response,
                ups_body,
                clt_send_response,
            )
            .await
        }
    }
}

pub enum RespmodAdaptationEndState {
    OriginalTransferred,
    AdaptedTransferred(HttpAdaptedResponse),
}
