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

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use std::sync::Arc;
    use tokio::io::Result;
    use tokio_util::io::StreamReader;

    #[tokio::test]
    async fn single_read() {
        let data: &[u8] = &[
            0x16, //
            0x03, 0x01, // TLS 1.0
            0x00, 0x65, // Fragment Length, 101
            0x01, // Handshake Type - ClientHello
            0x00, 0x00, 0x61, // Message Length, 97
            0x03, 0x03, // TLS 1.2
            0x74, 0x90, 0x65, 0xea, 0xbb, 0x00, 0x5d, 0xf8, 0xdf, 0xd6, 0xde, 0x04, 0xf8, 0xd3,
            0x69, 0x02, 0xf5, 0x8c, 0x82, 0x50, 0x7a, 0x40, 0xf6, 0xf3, 0xbb, 0x18, 0xc0, 0xac,
            0x4f, 0x55, 0x9a, 0xda, // Random data, 32 bytes
            0x20, // Session ID Length
            0x57, 0x5a, 0x8d, 0x9c, 0xa3, 0x8e, 0x16, 0xbd, 0xb6, 0x6c, 0xe7, 0x35, 0x62, 0x63,
            0x7f, 0x51, 0x5f, 0x6e, 0x97, 0xf7, 0xf9, 0x85, 0xad, 0xf0, 0x2d, 0x3a, 0x72, 0x9d,
            0x71, 0x0b, 0xe1, 0x32, // Session ID, 32 bytes
            0x00, 0x04, // Cipher Suites Length
            0x13, 0x02, 0x13, 0x01, // Cipher Suites
            0x01, // Compression Methods Length
            0x00, // Compression Methods
            0x00, 0x14, // Extensions Length, 20
            0x00, 0x00, // Extension Type - Server Name
            0x00, 0x10, // Extension Length, 16
            0x00, 0x0e, // Server Name List Length, 14
            0x00, // Server Name Type - Domain
            0x00, 0x0b, // Server Name Length, 11
            b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'.', b'n', b'e', b't',
        ];

        let content = b"test body\n";
        let stream = tokio_stream::iter(vec![Result::Ok(Bytes::from_static(content))]);
        let mut stream = StreamReader::new(stream);

        let mut clt_r_buf = BytesMut::from(data);

        let upstream = parse_request(&mut stream, &mut clt_r_buf, 443)
            .await
            .unwrap();
        assert_eq!(
            upstream,
            UpstreamAddr::new(Host::Domain(Arc::from("example.net")), 443)
        );
    }

    #[tokio::test]
    async fn multi_read() {
        let data: &[u8] = &[
            0x16, //
            0x03, 0x01, // TLS 1.0
            0x00, 0x65, // Fragment Length, 101
        ];
        let data1: &[u8] = &[
            0x01, // Handshake Type - ClientHello
            0x00, 0x00, 0x61, // Message Length, 97
            0x03, 0x03, // TLS 1.2
            0x74, 0x90, 0x65, 0xea, 0xbb, 0x00, 0x5d, 0xf8, 0xdf, 0xd6, 0xde, 0x04, 0xf8, 0xd3,
            0x69, 0x02, 0xf5, 0x8c, 0x82, 0x50, 0x7a, 0x40, 0xf6, 0xf3, 0xbb, 0x18, 0xc0, 0xac,
            0x4f, 0x55, 0x9a, 0xda, // Random data, 32 bytes
            0x20, // Session ID Length
            0x57, 0x5a, 0x8d, 0x9c, 0xa3, 0x8e, 0x16, 0xbd, 0xb6, 0x6c, 0xe7, 0x35, 0x62, 0x63,
            0x7f, 0x51, 0x5f, 0x6e, 0x97, 0xf7, 0xf9, 0x85, 0xad, 0xf0, 0x2d, 0x3a, 0x72, 0x9d,
            0x71, 0x0b, 0xe1, 0x32, // Session ID, 32 bytes
            0x00, 0x04, // Cipher Suites Length
            0x13, 0x02, 0x13, 0x01, // Cipher Suites
            0x01, // Compression Methods Length
            0x00, // Compression Methods
        ];
        let data2: &[u8] = &[
            0x00, 0x14, // Extensions Length, 20
            0x00, 0x00, // Extension Type - Server Name
            0x00, 0x10, // Extension Length, 16
            0x00, 0x0e, // Server Name List Length, 14
            0x00, // Server Name Type - Domain
            0x00, 0x0b, // Server Name Length, 11
            b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'.', b'n', b'e', b't',
        ];

        let stream = tokio_stream::iter(vec![
            Result::Ok(Bytes::from_static(data1)),
            Result::Ok(Bytes::from_static(data2)),
        ]);
        let mut stream = StreamReader::new(stream);

        let mut clt_r_buf = BytesMut::from(data);

        let upstream = parse_request(&mut stream, &mut clt_r_buf, 443)
            .await
            .unwrap();
        assert_eq!(
            upstream,
            UpstreamAddr::new(Host::Domain(Arc::from("example.net")), 443)
        );
    }
}
