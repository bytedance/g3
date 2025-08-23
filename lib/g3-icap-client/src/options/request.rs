/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use bytes::BufMut;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use super::{IcapOptionsParseError, IcapServiceOptions};
use crate::{IcapClientConnection, IcapServiceConfig};

pub(crate) struct IcapOptionsRequest<'a> {
    config: &'a IcapServiceConfig,
}

impl<'a> IcapOptionsRequest<'a> {
    pub(crate) fn new(config: &'a IcapServiceConfig) -> Self {
        IcapOptionsRequest { config }
    }

    async fn send<W>(&self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut header = self.config.build_options_request();
        if self.config.icap_206_enable {
            header.put_slice(b"Allow: 204, 206\r\n");
        } else {
            header.put_slice(b"Allow: 204\r\n");
        }
        header.put_slice(b"\r\n");

        writer.write_all(&header).await
    }

    pub(crate) async fn get_options(
        &self,
        conn: &mut IcapClientConnection,
        max_header_size: usize,
    ) -> Result<IcapServiceOptions, IcapOptionsParseError> {
        self.send(&mut conn.writer)
            .await
            .map_err(IcapOptionsParseError::IoFailed)?;
        conn.mark_writer_finished();

        let mut options =
            IcapServiceOptions::parse(&mut conn.reader, self.config.method, max_header_size)
                .await?;
        conn.mark_reader_finished();

        if !self.config.icap_206_enable {
            options.support_206 = false;
        }
        Ok(options)
    }
}
