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

use super::{KeylessHeader, KeylessRecvMessageError};

pub(crate) struct KeylessUpstreamResponse {
    header: KeylessHeader,
    payload: Vec<u8>,
}

impl KeylessUpstreamResponse {
    pub(crate) fn id(&self) -> u32 {
        self.header.id()
    }

    pub(crate) fn refresh(self, mut header: KeylessHeader) -> Self {
        header.sync_payload_len(&self.header);
        KeylessUpstreamResponse {
            header,
            payload: self.payload,
        }
    }

    pub(crate) async fn recv<R>(reader: &mut R) -> Result<Self, KeylessRecvMessageError>
    where
        R: AsyncRead + Unpin,
    {
        let mut header = KeylessHeader::default();
        let nr = reader.read_exact(header.as_mut()).await?;
        if nr == 0 {
            return Err(KeylessRecvMessageError::IoClosed);
        }
        if nr != super::KEYLESS_HEADER_LEN {
            return Err(KeylessRecvMessageError::InvalidHeaderLength(nr));
        }

        let len = header.payload_len() as usize;
        let mut payload = vec![0; len];
        let nr = reader.read_exact(payload.as_mut()).await?;
        if nr != len {
            return Err(KeylessRecvMessageError::InvalidPayloadLength(nr, len));
        }

        Ok(KeylessUpstreamResponse { header, payload })
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

#[derive(Clone, Copy)]
pub(crate) struct KeylessInternalErrorResponse {
    buf: [u8; super::KEYLESS_HEADER_LEN + 8],
}

impl KeylessInternalErrorResponse {
    pub(crate) fn new(header: KeylessHeader) -> Self {
        let h = header.as_ref();
        KeylessInternalErrorResponse {
            buf: [
                h[0], h[1], // protocol version
                0x00, 0x08, // message length
                h[4], h[5], h[6], h[7], // message id
                0x11, 0x00, 0x01, 0xFF, // OpCode - Error
                0x12, 0x00, 0x01, 0x08, // Payload - Internal Error
            ],
        }
    }

    pub(crate) async fn send<W>(&self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        writer.write_all(&self.buf).await?;
        writer.flush().await
    }
}

pub(crate) enum KeylessResponse {
    Upstream(KeylessUpstreamResponse),
    Local(KeylessInternalErrorResponse),
}

impl KeylessResponse {
    pub(crate) fn not_implemented(header: KeylessHeader) -> Self {
        KeylessResponse::Local(KeylessInternalErrorResponse::new(header))
    }

    pub(crate) async fn send<W>(&self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        match self {
            KeylessResponse::Upstream(u) => u.send(writer).await,
            KeylessResponse::Local(l) => l.send(writer).await,
        }
    }
}
