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
        self.send(&mut conn.0)
            .await
            .map_err(IcapOptionsParseError::IoFailed)?;
        let mut options =
            IcapServiceOptions::parse(&mut conn.1, self.config.method, max_header_size).await?;
        if !self.config.icap_206_enable {
            options.support_206 = false;
        }
        Ok(options)
    }
}
