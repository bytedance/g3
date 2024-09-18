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

use thiserror::Error;

use super::{HandshakeHeader, HandshakeType};
use crate::parser::tls::{ExtensionList, ExtensionParseError, ExtensionType, RawVersion};

#[derive(Debug, Error)]
pub enum ClientHelloParseError {
    #[error("invalid content type {0}")]
    InvalidContentType(u8),
    #[error("invalid fragment length")]
    InvalidFragmentLength,
    #[error("invalid message type {0}")]
    InvalidMessageType(u8),
    #[error("invalid message length")]
    InvalidMessageLength,
    #[error("invalid cipher suites length")]
    InvalidCipherSuitesLength,
    #[error("unsupported legacy version {0:?}")]
    UnsupportedVersion(RawVersion),
}

pub struct ClientHello<'a> {
    pub legacy_version: RawVersion,
    pub cipher_suites: &'a [u8],
    pub compression_methods: Option<&'a [u8]>,
    pub extensions: Option<&'a [u8]>,
}

impl<'a> ClientHello<'a> {
    pub fn parse_fragment(
        handshake_header: HandshakeHeader,
        data: &'a [u8],
    ) -> Result<Self, ClientHelloParseError> {
        if handshake_header.msg_type != HandshakeType::ClientHello as u8 {
            return Err(ClientHelloParseError::InvalidMessageType(
                handshake_header.msg_type,
            ));
        }
        let expected_data_len = handshake_header.msg_length as usize + HandshakeHeader::SIZE;
        if expected_data_len > data.len() {
            return Err(ClientHelloParseError::InvalidMessageLength);
        }

        Self::parse_msg_data(&data[HandshakeHeader::SIZE..])
    }

    pub(crate) fn parse_msg_data(data: &'a [u8]) -> Result<Self, ClientHelloParseError> {
        if data.len() < 2 {
            return Err(ClientHelloParseError::InvalidMessageLength);
        }

        let legacy_version = RawVersion {
            major: data[0],
            minor: data[1],
        };
        match (data[0], data[1]) {
            (1, 1) => {} // TLCP 1.1
            (3, 0) => {} // SSL 3.0
            (3, 1) => {} // TLS 1.0
            (3, 2) => {} // TLS 1.1
            (3, 3) => {} // TLS 1.2 and TLS 1.3
            _ => return Err(ClientHelloParseError::UnsupportedVersion(legacy_version)),
        }

        // Random Data
        let mut offset = 2;
        let left = &data[offset..];
        if left.len() < 32 {
            return Err(ClientHelloParseError::InvalidMessageLength);
        }
        offset += 32;

        // Session ID
        let left = &data[offset..];
        if left.is_empty() {
            return Err(ClientHelloParseError::InvalidMessageLength);
        }
        let session_id_len = left[0] as usize;
        if left.len() < 1 + session_id_len {
            return Err(ClientHelloParseError::InvalidMessageLength);
        }
        offset += 1 + session_id_len;

        // Cipher Suites
        let left = &data[offset..];
        if left.len() < 2 {
            return Err(ClientHelloParseError::InvalidMessageLength);
        }
        let cipher_suites_len = u16::from_be_bytes([left[0], left[1]]) as usize;
        if cipher_suites_len == 0 || cipher_suites_len & 0x01 != 0 {
            return Err(ClientHelloParseError::InvalidCipherSuitesLength);
        }
        if left.len() < 2 + cipher_suites_len {
            return Err(ClientHelloParseError::InvalidMessageLength);
        }
        let start = offset + 2;
        let end = start + cipher_suites_len;
        let cipher_suites = &data[start..end];
        offset = end;

        // Compression Methods
        let left = &data[offset..];
        if left.is_empty() {
            return Err(ClientHelloParseError::InvalidMessageLength);
        }
        let compression_methods_len = left[0] as usize;
        let compression_methods = if compression_methods_len > 0 {
            if left.len() < 1 + compression_methods_len {
                return Err(ClientHelloParseError::InvalidMessageLength);
            }
            let start = offset + 1;
            let end = start + compression_methods_len;
            offset = end;
            Some(&data[start..end])
        } else {
            offset += 1;
            None
        };

        if data.len() <= offset {
            // No Extensions
            return Ok(ClientHello {
                legacy_version,
                cipher_suites,
                compression_methods,
                extensions: None,
            });
        }

        // Extensions
        let left = &data[offset..];
        if left.len() < 2 {
            return Err(ClientHelloParseError::InvalidMessageLength);
        }
        let extensions_len = u16::from_be_bytes([left[0], left[1]]) as usize;
        let extensions = if extensions_len > 0 {
            if left.len() < 2 + extensions_len {
                return Err(ClientHelloParseError::InvalidMessageLength);
            }
            let start = offset + 2;
            let end = start + extensions_len;
            offset = end;
            Some(&data[start..end])
        } else {
            offset += 2;
            None
        };
        if data.len() > offset {
            return Err(ClientHelloParseError::InvalidMessageLength);
        }

        Ok(ClientHello {
            legacy_version,
            cipher_suites,
            compression_methods,
            extensions,
        })
    }

    pub fn get_ext(&self, ext_type: ExtensionType) -> Result<Option<&[u8]>, ExtensionParseError> {
        let Some(data) = self.extensions else {
            return Ok(None);
        };

        ExtensionList::get_ext(data, ext_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::tls::{HandshakeCoalescer, Record};
    use g3_types::net::TlsServerName;

    #[test]
    fn tls1_2() {
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

        let mut handshake_coalescer = HandshakeCoalescer::default();
        let mut record = Record::parse(data).unwrap();
        let handshake_msg = record
            .consume_handshake(&mut handshake_coalescer)
            .unwrap()
            .unwrap();
        let ch = handshake_msg.parse_client_hello().unwrap();
        let sni_data = ch.get_ext(ExtensionType::ServerName).unwrap().unwrap();
        let sni = TlsServerName::from_extension_value(sni_data).unwrap();
        assert!(record.consume_done());
        assert_eq!(sni.as_ref(), "example.net");
    }

    #[test]
    fn tlcp() {
        let data: &[u8] = &[
            0x16, //
            0x01, 0x01, // TLCP 1.1
            0x00, 0x65, // Fragment Length, 101
            0x01, // Handshake Type - ClientHello
            0x00, 0x00, 0x61, // Message Length, 97
            0x01, 0x01, // TLCP 1.1
            0x74, 0x90, 0x65, 0xea, 0xbb, 0x00, 0x5d, 0xf8, 0xdf, 0xd6, 0xde, 0x04, 0xf8, 0xd3,
            0x69, 0x02, 0xf5, 0x8c, 0x82, 0x50, 0x7a, 0x40, 0xf6, 0xf3, 0xbb, 0x18, 0xc0, 0xac,
            0x4f, 0x55, 0x9a, 0xda, // Random data, 32 bytes
            0x20, // Session ID Length
            0x57, 0x5a, 0x8d, 0x9c, 0xa3, 0x8e, 0x16, 0xbd, 0xb6, 0x6c, 0xe7, 0x35, 0x62, 0x63,
            0x7f, 0x51, 0x5f, 0x6e, 0x97, 0xf7, 0xf9, 0x85, 0xad, 0xf0, 0x2d, 0x3a, 0x72, 0x9d,
            0x71, 0x0b, 0xe1, 0x32, // Session ID, 32 bytes
            0x00, 0x04, // Cipher Suites Length
            0xe0, 0x11, 0x00, 0xff, // Cipher Suites
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

        let mut handshake_coalescer = HandshakeCoalescer::default();
        let mut record = Record::parse(data).unwrap();
        let handshake_msg = record
            .consume_handshake(&mut handshake_coalescer)
            .unwrap()
            .unwrap();
        let ch = handshake_msg.parse_client_hello().unwrap();
        let sni_data = ch.get_ext(ExtensionType::ServerName).unwrap().unwrap();
        let sni = TlsServerName::from_extension_value(sni_data).unwrap();
        assert!(record.consume_done());
        assert_eq!(sni.as_ref(), "example.net");
    }

    #[test]
    fn invalid_ext_len() {
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
            0x01, 0x10, // Extension Length, 256 + 16
            0x00, 0x0e, // Server Name List Length, 14
            0x00, // Server Name Type - Domain
            0x00, 0x0b, // Server Name Length, 11
            b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'.', b'n', b'e', b't',
        ];

        let mut handshake_coalescer = HandshakeCoalescer::default();
        let mut record = Record::parse(data).unwrap();
        let handshake_msg = record
            .consume_handshake(&mut handshake_coalescer)
            .unwrap()
            .unwrap();
        let ch = handshake_msg.parse_client_hello().unwrap();
        assert!(ch.get_ext(ExtensionType::ServerName).is_err());
        assert!(record.consume_done());
    }

    #[test]
    fn invalid_ext_list_len() {
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
            0x01, 0x14, // Extensions Length, 256 + 20
            0x00, 0x00, // Extension Type - Server Name
            0x00, 0x10, // Extension Length, 16
            0x00, 0x0e, // Server Name List Length, 14
            0x00, // Server Name Type - Domain
            0x00, 0x0b, // Server Name Length, 11
            b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'.', b'n', b'e', b't',
        ];

        let mut handshake_coalescer = HandshakeCoalescer::default();
        let mut record = Record::parse(data).unwrap();
        let handshake_msg = record
            .consume_handshake(&mut handshake_coalescer)
            .unwrap()
            .unwrap();
        assert!(handshake_msg.parse_client_hello().is_err());
        assert!(record.consume_done());
    }
}
