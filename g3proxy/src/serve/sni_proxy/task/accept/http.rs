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

use bytes::BytesMut;
use tokio::io::{AsyncRead, AsyncReadExt};

use g3_http::server::HttpTransparentRequestAcceptor;
use g3_types::net::UpstreamAddr;

use crate::serve::{ServerTaskError, ServerTaskResult};

pub(super) async fn parse_request<R>(
    clt_r: &mut R,
    clt_r_buf: &mut BytesMut,
    port: u16,
) -> ServerTaskResult<UpstreamAddr>
where
    R: AsyncRead + Unpin,
{
    let mut acceptor = HttpTransparentRequestAcceptor::default();

    let mut read_offset = 0;
    loop {
        let b = &clt_r_buf[read_offset..];
        let nr = acceptor
            .read_http(b)
            .map_err(|_e| ServerTaskError::InvalidClientProtocol("invalid http request"))?;
        read_offset += nr;

        match acceptor.accept() {
            Some(req) => {
                let mut host = req.host.ok_or(ServerTaskError::InvalidClientProtocol(
                    "no host header found in http request",
                ))?;
                if host.port() == 0 {
                    host.set_port(port);
                }
                return Ok(host);
            }
            None => match clt_r.read_buf(clt_r_buf).await {
                Ok(0) => return Err(ServerTaskError::ClosedByClient),
                Ok(_) => {}
                Err(e) => return Err(ServerTaskError::ClientTcpReadFailed(e)),
            },
        }
    }
}
