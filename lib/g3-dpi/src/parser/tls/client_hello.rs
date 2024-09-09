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
    HandshakeHeader, HandshakeParseError, HandshakeType, RawVersion, RecordHeader, RecordParseError,
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
    #[error("unsupported legacy version {0:?}")]
    UnsupportedVersion(RawVersion),
}

pub struct ClientHello {
    pub record_header: RecordHeader,
    pub legacy_version: RawVersion,
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

impl ClientHello {
    pub fn parse(data: &[u8]) -> Result<Self, ClientHelloParseError> {
        let record_header = RecordHeader::parse(data)?;
        if data.len() != record_header.payload_len as usize + RecordHeader::SIZE {
            return Err(ClientHelloParseError::NeedMoreData(
                record_header.payload_len as usize + RecordHeader::SIZE - data.len(),
            ));
        }
        if record_header.payload_len & 0b1100_0000_0000_0000 != 0 {
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
            != record_header.payload_len as usize
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

        Ok(ClientHello {
            record_header,
            legacy_version,
        })
    }
}
