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

use g3_dpi::parser::tls::{ClientHello, ClientHelloParseError, ExtensionType};
use g3_types::net::{Host, TlsServerName, UpstreamAddr};

use crate::serve::{ServerTaskError, ServerTaskResult};

pub(super) async fn parse_request<R>(
    clt_r: &mut R,
    clt_r_buf: &mut BytesMut,
    port: u16,
) -> ServerTaskResult<UpstreamAddr>
where
    R: AsyncRead + Unpin,
{
    loop {
        match ClientHello::parse(clt_r_buf) {
            Ok(ch) => match ch.get_ext(ExtensionType::ServerName) {
                Ok(Some(data)) => {
                    let sni = TlsServerName::from_extension_value(data).map_err(|_| {
                        ServerTaskError::InvalidClientProtocol(
                            "invalid server name in tls client hello message",
                        )
                    })?;
                    return Ok(UpstreamAddr::new(Host::from(sni), port));
                }
                Ok(None) => {
                    return Err(ServerTaskError::InvalidClientProtocol(
                        "no server name found in tls client hello message",
                    ));
                }
                Err(_) => {
                    return Err(ServerTaskError::InvalidClientProtocol(
                        "invalid extension in tls client hello request",
                    ));
                }
            },
            Err(ClientHelloParseError::NeedMoreData(_)) => match clt_r.read_buf(clt_r_buf).await {
                Ok(0) => return Err(ServerTaskError::ClosedByClient),
                Ok(_) => {}
                Err(e) => return Err(ServerTaskError::ClientTcpReadFailed(e)),
            },
            Err(_) => {
                return Err(ServerTaskError::InvalidClientProtocol(
                    "invalid tls client hello request",
                ));
            }
        }
    }
}
