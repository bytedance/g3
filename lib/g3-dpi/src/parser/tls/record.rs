/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
    Handshake = 22,
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
    #[error("need {0} bytes more data")]
    NeedMoreData(usize),
    #[error("unsupported protocol version {0:?}")]
    UnsupportedVersion(RawVersion),
    #[error("invalid content type {0}")]
    InvalidContentType(u8),
    #[error("fragment length exceeded")]
    FragmentLengthExceeded,
}

pub struct RecordHeader {
    pub version: RawVersion,
    pub content_type: ContentType,
    pub fragment_size: u16,
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
            fragment_size: fragment_len,
        })
    }
}

pub struct Record<'a> {
    header: RecordHeader,
    fragment: &'a [u8],
    consume_offset: usize,
}

impl<'a> Record<'a> {
    /// Parse a TLS Record
    ///
    /// According to https://datatracker.ietf.org/doc/html/rfc8446#section-5.1
    pub fn parse(data: &'a [u8]) -> Result<Self, RecordParseError> {
        if data.len() < RecordHeader::SIZE {
            return Err(RecordParseError::NeedMoreData(
                RecordHeader::SIZE - data.len(),
            ));
        }

        let header = RecordHeader::parse(data)?;
        if header.fragment_size > 1 << 14 {
            // The length MUST NOT exceed 2^14 bytes.
            return Err(RecordParseError::FragmentLengthExceeded);
        }

        let start = RecordHeader::SIZE;
        let end = start + header.fragment_size as usize;
        if data.len() < end {
            return Err(RecordParseError::NeedMoreData(end - data.len()));
        }

        Ok(Record {
            header,
            fragment: &data[start..end],
            consume_offset: 0,
        })
    }

    /// Get the total length of this record on the wire
    pub fn encoded_len(&self) -> usize {
        RecordHeader::SIZE + self.fragment.len()
    }

    /// Consume the fragment data as a Handshake message
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

        if coalescer.is_empty()
            && let Some(msg) = HandshakeMessage::try_parse_fragment(fragment)
        {
            self.consume_offset += msg.encoded_len();
            return Ok(Some(msg));
        }

        self.consume_offset += coalescer.coalesce_fragment(fragment)?;
        Ok(None)
    }

    /// Check if all fragment data is consumed
    pub fn consume_done(&self) -> bool {
        self.consume_offset >= self.fragment.len()
    }
}
