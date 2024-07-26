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

use std::io::{self, IoSlice};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use g3_io_ext::LimitedWriteExt;

use super::KeylessHeader;
use crate::serve::ServerTaskError;

pub(crate) struct KeylessRequest {
    header: KeylessHeader,
    payload: Vec<u8>,
}

impl KeylessRequest {
    #[inline]
    pub(crate) fn header(&self) -> KeylessHeader {
        self.header
    }

    pub(crate) async fn recv<R>(reader: &mut R) -> Result<Self, ServerTaskError>
    where
        R: AsyncRead + Unpin,
    {
        let mut header = KeylessHeader::default();
        let nr = reader
            .read_exact(header.as_mut())
            .await
            .map_err(ServerTaskError::ClientTcpReadFailed)?;
        if nr == 0 {
            return Err(ServerTaskError::ClosedByClient);
        }
        if nr != super::KEYLESS_HEADER_LEN {
            return Err(ServerTaskError::InvalidClientProtocol(
                "invalid keyless header length",
            ));
        }

        let len = header.payload_len() as usize;
        let mut payload = vec![0; len];
        let nr = reader
            .read_exact(payload.as_mut())
            .await
            .map_err(ServerTaskError::ClientTcpReadFailed)?;
        if nr != len {
            return Err(ServerTaskError::InvalidClientProtocol(
                "invalid keyless payload length",
            ));
        }

        Ok(KeylessRequest { header, payload })
    }

    pub(crate) fn refresh(mut self, id: u32) -> Self {
        self.header.set_id(id);
        self
    }

    pub(crate) async fn send<W>(&self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        writer
            .write_all_vectored([
                IoSlice::new(self.header.as_ref()),
                IoSlice::new(&self.payload),
            ])
            .await?;
        writer.flush().await
    }
}
