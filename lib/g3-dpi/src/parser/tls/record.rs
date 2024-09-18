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

use super::{HandshakeCoalescer, HandshakeMessage, RawVersion};
use crate::parser::tls::handshake::HandshakeCoalesceError;

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
    #[error("invalid fragment length")]
    InvalidFragmentLength,
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
    fragment: &'a [u8],
    consume_offset: usize,
}

impl<'a> Record<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, RecordParseError> {
        if data.len() < RecordHeader::SIZE {
            return Err(RecordParseError::NeedMoreData(
                RecordHeader::SIZE - data.len(),
            ));
        }

        let header = RecordHeader::parse(data)?;
        if header.fragment_len & 0b1100_0000_0000_0000 != 0 {
            // The length MUST NOT exceed 2^14 bytes.
            return Err(RecordParseError::InvalidFragmentLength);
        }

        let start = RecordHeader::SIZE;
        let end = start + header.fragment_len as usize;
        if data.len() < end {
            return Err(RecordParseError::NeedMoreData(end - data.len()));
        }

        Ok(Record {
            header,
            fragment: &data[start..end],
            consume_offset: 0,
        })
    }

    pub fn encoded_len(&self) -> usize {
        RecordHeader::SIZE + self.fragment.len()
    }

    pub fn consume_handshake(
        &mut self,
        coalescer: &mut HandshakeCoalescer,
    ) -> Result<Option<HandshakeMessage<'_>>, HandshakeCoalesceError> {
        if self.header.content_type != ContentType::Handshake {
            return Err(HandshakeCoalesceError::InvalidContentType(
                self.header.content_type as u8,
            ));
        }

        if self.consume_done() {
            return Ok(None);
        }
        let fragment = &self.fragment[self.consume_offset..];

        if coalescer.is_empty() {
            if let Some(msg) = HandshakeMessage::parse_fragment(fragment) {
                self.consume_offset += msg.encoded_len();
                return Ok(Some(msg));
            }
        }

        self.consume_offset += coalescer.coalesce_fragment(fragment)?;
        Ok(None)
    }

    pub fn consume_done(&self) -> bool {
        self.consume_offset >= self.fragment.len()
    }
}
