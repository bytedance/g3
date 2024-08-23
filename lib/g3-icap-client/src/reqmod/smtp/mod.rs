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

use bytes::BufMut;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_io_ext::{IdleCheck, LimitedCopyConfig};
use g3_smtp_proto::command::{MailParam, RecipientParam};

use super::IcapReqmodClient;
use crate::reqmod::mail::{ReqmodAdaptationEndState, ReqmodAdaptationRunState};
use crate::{IcapClientConnection, IcapServiceClient};

pub use crate::reqmod::h1::HttpAdapterErrorResponse;

mod error;
pub use error::SmtpAdaptationError;

mod data;

impl IcapReqmodClient {
    pub async fn smtp_message_adaptor<I: IdleCheck>(
        &self,
        copy_config: LimitedCopyConfig,
        idle_checker: I,
    ) -> anyhow::Result<SmtpMessageAdapter<I>> {
        let icap_client = self.inner.clone();
        let (icap_connection, _icap_options) = icap_client.fetch_connection().await?;
        Ok(SmtpMessageAdapter {
            icap_client,
            icap_connection,
            copy_config,
            idle_checker,
            client_addr: None,
            client_username: None,
        })
    }
}

pub struct SmtpMessageAdapter<I: IdleCheck> {
    icap_client: Arc<IcapServiceClient>,
    icap_connection: IcapClientConnection,
    copy_config: LimitedCopyConfig,
    // TODO add SMTP config
    idle_checker: I,
    client_addr: Option<SocketAddr>,
    client_username: Option<Arc<str>>,
}

impl<I: IdleCheck> SmtpMessageAdapter<I> {
    pub fn set_client_addr(&mut self, addr: SocketAddr) {
        self.client_addr = Some(addr);
    }

    pub fn set_client_username(&mut self, user: Arc<str>) {
        self.client_username = Some(user);
    }

    pub fn build_http_header(&self, mail_from: &MailParam, mail_to: &[RecipientParam]) -> Vec<u8> {
        let mut header = Vec::with_capacity(128);
        header.extend_from_slice(b"PUT / HTTP/1.1\r\n");
        header.extend_from_slice(b"Content-Type: message/rfc822\r\n");
        let _ = write!(&mut header, "X-SMTP-From: {}\r\n", mail_from.reverse_path());
        for to in mail_to {
            let _ = write!(&mut header, "X-SMTP-To: {}\r\n", to.forward_path());
        }
        header.extend_from_slice(b"\r\n");
        header
    }

    fn push_extended_headers(&self, data: &mut Vec<u8>) {
        data.put_slice(b"X-Transformed-From: SMTP\r\n");
        if let Some(addr) = self.client_addr {
            crate::serialize::add_client_addr(data, addr);
        }
        if let Some(user) = &self.client_username {
            crate::serialize::add_client_username(data, user);
        }
    }

    pub async fn xfer_data<CR, UW>(
        self,
        state: &mut ReqmodAdaptationRunState,
        clt_r: &mut CR,
        ups_w: &mut UW,
        mail_from: &MailParam,
        mail_to: &[RecipientParam],
    ) -> Result<ReqmodAdaptationEndState, SmtpAdaptationError>
    where
        CR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        // TODO support preview?
        self.xfer_data_without_preview(state, clt_r, ups_w, mail_from, mail_to)
            .await
    }
}
