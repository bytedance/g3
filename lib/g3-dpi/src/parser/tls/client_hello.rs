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

use super::{
    ExtensionList, ExtensionParseError, ExtensionType, HandshakeHeader, HandshakeParseError,
    HandshakeType, RawVersion, RecordHeader, RecordParseError,
};

#[derive(Debug, Error)]
pub enum ClientHelloParseError {
    #[error("need more data {0}")]
    NeedMoreData(usize),
    #[error("invalid tls record: {0}")]
    InvalidTlsRecord(RecordParseError),
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
    pub record_header: RecordHeader,
    pub legacy_version: RawVersion,
    pub cipher_suites: &'a [u8],
    pub compression_methods: Option<&'a [u8]>,
    pub extensions: Option<&'a [u8]>,
}

impl From<RecordParseError> for ClientHelloParseError {
    fn from(value: RecordParseError) -> Self {
        if let RecordParseError::InvalidDataSize(l) = value {
            ClientHelloParseError::NeedMoreData(RecordHeader::SIZE - l)
        } else {
            ClientHelloParseError::InvalidTlsRecord(value)
        }
    }
}

impl From<HandshakeParseError> for ClientHelloParseError {
    fn from(value: HandshakeParseError) -> Self {
        match value {
            HandshakeParseError::InvalidDataSize(l) => {
                ClientHelloParseError::NeedMoreData(HandshakeHeader::SIZE - l)
            }
        }
    }
}

impl<'a> ClientHello<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, ClientHelloParseError> {
        let record_header = RecordHeader::parse(data)?;
        if data.len() != record_header.fragment_len as usize + RecordHeader::SIZE {
            return Err(ClientHelloParseError::NeedMoreData(
                record_header.fragment_len as usize + RecordHeader::SIZE - data.len(),
            ));
        }
        if record_header.fragment_len & 0b1100_0000_0000_0000 != 0 {
            // The length MUST NOT exceed 2^14 bytes.
            return Err(ClientHelloParseError::InvalidFragmentLength);
        }

        let mut offset = RecordHeader::SIZE;
        let handshake_header = HandshakeHeader::parse(&data[offset..])?;
        if handshake_header.msg_type != HandshakeType::ClientHello as u8 {
            return Err(ClientHelloParseError::InvalidMessageType(
                handshake_header.msg_type,
            ));
        }
        if handshake_header.msg_length as usize + HandshakeHeader::SIZE
            != record_header.fragment_len as usize
        {
            return Err(ClientHelloParseError::InvalidMessageLength);
        }

        offset += HandshakeHeader::SIZE;
        let msg = &data[offset..];
        if msg.len() < 2 {
            return Err(ClientHelloParseError::InvalidMessageLength);
        }

        let legacy_version = RawVersion {
            major: msg[0],
            minor: msg[1],
        };
        match (msg[1], msg[2]) {
            (1, 1) => {} // TLCP 1.1
            (3, 0) => {} // SSL 3.0
            (3, 1) => {} // TLS 1.0
            (3, 2) => {} // TLS 1.1
            (3, 3) => {} // TLS 1.2 and TLS 1.3
            _ => return Err(ClientHelloParseError::UnsupportedVersion(legacy_version)),
        }

        // Random Data
        offset += 2;
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
                record_header,
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
            record_header,
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
