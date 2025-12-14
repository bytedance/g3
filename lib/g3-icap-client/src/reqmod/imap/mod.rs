/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;

use arcstr::ArcStr;
use bytes::BufMut;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_io_ext::{IdleCheck, StreamCopyConfig};

use super::IcapReqmodClient;
use crate::reqmod::mail::{ReqmodAdaptationEndState, ReqmodAdaptationRunState};
use crate::{IcapClientConnection, IcapServiceClient, IcapServiceOptions};

pub use crate::reqmod::h1::HttpAdapterErrorResponse;

mod error;
pub use error::ImapAdaptationError;

mod append;

impl IcapReqmodClient {
    pub async fn imap_message_adaptor<I: IdleCheck>(
        &self,
        copy_config: StreamCopyConfig,
        idle_checker: I,
        literal_size: u64,
    ) -> anyhow::Result<ImapMessageAdapter<I>> {
        let icap_client = self.inner.clone();
        let (icap_connection, icap_options) = icap_client.fetch_connection().await?;
        Ok(ImapMessageAdapter {
            icap_client,
            icap_connection,
            icap_options,
            copy_config,
            idle_checker,
            client_addr: None,
            client_username: None,
            literal_size,
        })
    }
}

pub struct ImapMessageAdapter<I: IdleCheck> {
    icap_client: Arc<IcapServiceClient>,
    icap_connection: IcapClientConnection,
    icap_options: Arc<IcapServiceOptions>,
    copy_config: StreamCopyConfig,
    idle_checker: I,
    client_addr: Option<SocketAddr>,
    client_username: Option<ArcStr>,
    literal_size: u64,
}

impl<I: IdleCheck> ImapMessageAdapter<I> {
    pub fn set_client_addr(&mut self, addr: SocketAddr) {
        self.client_addr = Some(addr);
    }

    pub fn set_client_username(&mut self, user: ArcStr) {
        self.client_username = Some(user);
    }

    pub fn build_http_header(&self) -> Vec<u8> {
        let mut header = Vec::with_capacity(128);
        header.extend_from_slice(b"PUT / HTTP/1.1\r\n");
        header.extend_from_slice(b"Content-Type: message/rfc822\r\n");

        let mut len_buf = itoa::Buffer::new();
        let len_s = len_buf.format(self.literal_size);

        header.extend_from_slice(b"Content-Length: ");
        header.extend_from_slice(len_s.as_bytes());
        header.extend_from_slice(b"\r\n");

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
        cached: &[u8],
        ups_w: &mut UW,
    ) -> Result<ReqmodAdaptationEndState, ImapAdaptationError>
    where
        CR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        if self.literal_size > cached.len() as u64 {
            // TODO support preview?

            let read_size = self.literal_size - cached.len() as u64;
            self.xfer_append_without_preview(state, clt_r, cached, read_size, ups_w)
                .await
        } else {
            self.xfer_append_once(state, cached, ups_w).await
        }
    }
}
