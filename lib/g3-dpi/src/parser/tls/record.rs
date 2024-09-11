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

use super::{ClientHello, ClientHelloParseError, RawVersion};

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ContentType {
    Invalid = 0, // TLS 1.3
    ChangeCipherSpec = 20,
    Alert = 21,
    Handshake = 22, // 0x16
    ApplicationData = 23,
    Heartbeat = 24, // RFC 6520
}

impl TryFrom<u8> for ContentType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            20 => Ok(ContentType::ChangeCipherSpec),
            21 => Ok(ContentType::Alert),
            22 => Ok(ContentType::Handshake),
            23 => Ok(ContentType::ApplicationData),
            24 => Ok(ContentType::Heartbeat),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Error)]
pub enum RecordParseError {
    #[error("need more data of size {0}")]
    NeedMoreData(usize),
    #[error("unsupported protocol version {0:?}")]
    UnsupportedVersion(RawVersion),
    #[error("invalid content type {0}")]
    InvalidContentType(u8),
}

pub struct RecordHeader {
    pub version: RawVersion,
    pub content_type: ContentType,
    pub fragment_len: u16,
}

impl RecordHeader {
    pub const SIZE: usize = 5;

    fn parse(data: &[u8]) -> Result<Self, RecordParseError> {
        let Ok(content_type) = ContentType::try_from(data[0]) else {
            return Err(RecordParseError::InvalidContentType(data[0]));
        };

        let version = RawVersion {
            major: data[1],
            minor: data[2],
        };
        match (data[1], data[2]) {
            (1, 1) => {} // TLCP 1.1
            (3, 0) => {} // SSL 3.0
            (3, 1) => {} // TLS 1.0
            (3, 2) => {} // TLS 1.1
            (3, 3) => {} // TLS 1.2 and TLS 1.3
            _ => return Err(RecordParseError::UnsupportedVersion(version)),
        }

        let fragment_len = u16::from_be_bytes([data[3], data[4]]);
        Ok(RecordHeader {
            version,
            content_type,
            fragment_len,
        })
    }
}

pub struct Record<'a> {
    pub header: RecordHeader,
    pub fragment: &'a [u8],
}

impl<'a> Record<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, RecordParseError> {
        if data.len() < RecordHeader::SIZE {
            return Err(RecordParseError::NeedMoreData(
                RecordHeader::SIZE - data.len(),
            ));
        }

        let header = RecordHeader::parse(data)?;

        let start = RecordHeader::SIZE;
        let end = start + header.fragment_len as usize;
        if data.len() < end {
            return Err(RecordParseError::NeedMoreData(end - data.len()));
        }

        Ok(Record {
            header,
            fragment: &data[start..end],
        })
    }

    pub fn parse_client_hello(&'a self) -> Result<ClientHello<'a>, ClientHelloParseError> {
        if self.header.content_type != ContentType::Handshake {
            return Err(ClientHelloParseError::InvalidContentType(
                self.header.content_type as u8,
            ));
        }
        if self.header.fragment_len & 0b1100_0000_0000_0000 != 0 {
            // The length MUST NOT exceed 2^14 bytes.
            return Err(ClientHelloParseError::InvalidFragmentLength);
        }
        ClientHello::parse_fragment(self.fragment)
    }
}
