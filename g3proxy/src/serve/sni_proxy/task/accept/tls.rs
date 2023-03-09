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
    let mut acceptor = rustls::server::Acceptor::default();

    let mut read_tls_offset = 0;
    loop {
        let mut b = &clt_r_buf[read_tls_offset..];
        let tls_nr = acceptor.read_tls(&mut b).map_err(|_e| {
            ServerTaskError::InvalidClientProtocol("invalid tls client hello request")
        })?;
        read_tls_offset += tls_nr;

        match acceptor.accept() {
            Ok(Some(accepted)) => {
                let client_hello = accepted.client_hello();
                let sni =
                    client_hello
                        .server_name()
                        .ok_or(ServerTaskError::InvalidClientProtocol(
                            "no server name found in tls client hello message",
                        ))?;
                let upstream = UpstreamAddr::from_host_str_and_port(sni, port).map_err(|_e| {
                    ServerTaskError::InvalidClientProtocol(
                        "invalid server name in tls client hello message",
                    )
                })?;
                return Ok(upstream);
            }
            Ok(None) => match clt_r.read_buf(clt_r_buf).await {
                Ok(0) => return Err(ServerTaskError::ClosedByClient),
                Ok(_) => {}
                Err(e) => return Err(ServerTaskError::ClientTcpReadFailed(e)),
            },
            Err(_e) => {
                return Err(ServerTaskError::InvalidClientProtocol(
                    "invalid tls client hello request",
                ));
            }
        }
    }
}
