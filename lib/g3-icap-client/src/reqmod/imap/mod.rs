/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use bytes::BufMut;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite};
use tokio::time::Instant;

use g3_http::HttpBodyDecodeReader;
use g3_io_ext::{IdleCheck, LimitedCopyConfig};

use super::IcapReqmodClient;
use crate::{IcapClientConnection, IcapServiceClient};

pub use crate::reqmod::h1::HttpAdapterErrorResponse;

mod error;
pub use error::ImapAdaptationError;

mod append;

impl IcapReqmodClient {
    pub async fn mail_message_adaptor<I: IdleCheck>(
        &self,
        copy_config: LimitedCopyConfig,
        idle_checker: I,
    ) -> anyhow::Result<ImapMessageAdapter<I>> {
        let icap_client = self.inner.clone();
        let (icap_connection, _icap_options) = icap_client.fetch_connection().await?;
        Ok(ImapMessageAdapter {
            icap_client,
            icap_connection,
            copy_config,
            idle_checker,
            client_addr: None,
            client_username: None,
        })
    }
}

pub struct ReqmodAdaptationRunState {
    task_create_instant: Instant,
    pub dur_ups_send_all: Option<Duration>,
    pub clt_read_finished: bool,
    pub ups_write_finished: bool,
    pub(crate) icap_io_finished: bool,
}

impl ReqmodAdaptationRunState {
    pub fn new(task_create_instant: Instant) -> Self {
        ReqmodAdaptationRunState {
            task_create_instant,
            dur_ups_send_all: None,
            clt_read_finished: false,
            ups_write_finished: false,
            icap_io_finished: false,
        }
    }

    pub(crate) fn mark_ups_send_all(&mut self) {
        self.dur_ups_send_all = Some(self.task_create_instant.elapsed());
        self.ups_write_finished = true;
    }
}

pub struct ImapMessageAdapter<I: IdleCheck> {
    icap_client: Arc<IcapServiceClient>,
    icap_connection: IcapClientConnection,
    copy_config: LimitedCopyConfig,
    idle_checker: I,
    client_addr: Option<SocketAddr>,
    client_username: Option<String>,
}

impl<I: IdleCheck> ImapMessageAdapter<I> {
    pub fn set_client_addr(&mut self, addr: SocketAddr) {
        self.client_addr = Some(addr);
    }

    pub fn set_client_username(&mut self, user: &str) {
        self.client_username = Some(user.to_string());
    }

    pub fn build_http_header(&self, literal_size: u64) -> Vec<u8> {
        let mut header = Vec::with_capacity(128);
        header.extend_from_slice(b"PUT / HTTP/1.1\r\n");
        header.extend_from_slice(b"Content-Type: message/rfc822\r\n");
        let _ = write!(header, "X-IMAP-Message-Size: {literal_size}\r\n");
        header.extend_from_slice(b"\r\n");
        header
    }

    fn push_extended_headers(&self, data: &mut Vec<u8>) {
        data.put_slice(b"X-Transformed-From: IMAP\r\n");
        if let Some(addr) = self.client_addr {
            crate::serialize::add_client_addr(data, addr);
        }
        if let Some(user) = &self.client_username {
            crate::serialize::add_client_username(data, user);
        }
    }

    pub async fn xfer_append<CR, UW>(
        self,
        state: &mut ReqmodAdaptationRunState,
        clt_r: &mut CR,
        literal_size: u64,
        ups_w: &mut UW,
    ) -> Result<ReqmodAdaptationEndState, ImapAdaptationError>
    where
        CR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        // TODO support preview?
        self.xfer_append_without_preview(state, clt_r, literal_size, ups_w)
            .await
    }
}

pub enum ReqmodAdaptationEndState {
    OriginalTransferred,
    AdaptedTransferred,
    HttpErrResponse(HttpAdapterErrorResponse, Option<ReqmodRecvHttpResponseBody>),
}

pub struct ReqmodRecvHttpResponseBody {
    icap_client: Arc<IcapServiceClient>,
    icap_keepalive: bool,
    icap_connection: IcapClientConnection,
}

impl ReqmodRecvHttpResponseBody {
    pub fn body_reader(&mut self) -> HttpBodyDecodeReader<'_, impl AsyncBufRead> {
        HttpBodyDecodeReader::new_chunked(&mut self.icap_connection.1, 1024)
    }

    pub async fn save_connection(self) {
        if self.icap_keepalive {
            self.icap_client.save_connection(self.icap_connection).await;
        }
    }
}
