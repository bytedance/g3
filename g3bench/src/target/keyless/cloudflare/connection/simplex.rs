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

use std::net::SocketAddr;

use futures_util::FutureExt;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::{KeylessLocalError, KeylessRequest, KeylessResponse, KeylessResponseError};

pub(crate) struct SimplexTransfer {
    reader: Box<dyn AsyncRead + Send + Sync + Unpin>,
    writer: Box<dyn AsyncWrite + Send + Sync + Unpin>,
    next_req_id: u32,
    read_buf: Vec<u8>,
    local_addr: SocketAddr,
}

impl SimplexTransfer {
    pub(crate) fn new<R, W>(reader: R, writer: W, local_addr: SocketAddr) -> Self
    where
        R: AsyncRead + Send + Sync + Unpin + 'static,
        W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        SimplexTransfer {
            reader: Box::new(reader),
            writer: Box::new(writer),
            next_req_id: 0,
            read_buf: Vec::with_capacity(1024),
            local_addr,
        }
    }

    pub(crate) fn is_closed(&mut self) -> bool {
        let mut buf = [0u8; 4];
        self.reader.read(&mut buf).now_or_never().is_some()
    }

    #[inline]
    pub(crate) fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub(crate) async fn send_request(
        &mut self,
        req: &mut KeylessRequest,
    ) -> Result<KeylessResponse, KeylessResponseError> {
        req.set_id(self.next_req_id);
        self.next_req_id = self.next_req_id.wrapping_add(1);

        self.writer
            .write_all(req.as_bytes())
            .await
            .map_err(KeylessLocalError::WriteFailed)?;

        KeylessResponse::read(&mut self.reader, &mut self.read_buf).await
    }
}
