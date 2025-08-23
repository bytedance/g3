/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
