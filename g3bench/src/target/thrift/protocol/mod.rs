/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use thiserror::Error;

mod binary;
pub(super) use binary::BinaryMessageBuilder;
use binary::BinaryMessageParser;

mod compact;
pub(super) use compact::CompactMessageBuilder;
use compact::CompactMessageParser;

pub(super) enum ThriftProtocol {
    Binary,
    Compact,
}

pub(super) enum ThriftMessageBuilder {
    Binary(BinaryMessageBuilder),
    Compact(CompactMessageBuilder),
}

impl ThriftMessageBuilder {
    pub(super) fn build_call(
        &self,
        seq_id: i32,
        framed: bool,
        payload: &[u8],
        buf: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        match self {
            ThriftMessageBuilder::Binary(r) => r.build_call(seq_id, framed, payload, buf),
            ThriftMessageBuilder::Compact(r) => r.build_call(seq_id, framed, payload, buf),
        }
    }

    pub(super) fn protocol(&self) -> ThriftProtocol {
        match self {
            ThriftMessageBuilder::Binary(_) => ThriftProtocol::Binary,
            ThriftMessageBuilder::Compact(_) => ThriftProtocol::Compact,
        }
    }

    pub(super) fn response_parser(&self) -> ThriftResponseMessageParser {
        match self {
            ThriftMessageBuilder::Binary(_) => {
                ThriftResponseMessageParser::Binary(Default::default())
            }
            ThriftMessageBuilder::Compact(_) => {
                ThriftResponseMessageParser::Compact(Default::default())
            }
        }
    }
}

#[derive(Debug, Error)]
pub(super) enum ThriftResponseMessageParseError {
    #[error("no enough data")]
    NoEnoughData,
    #[error("invalid protocol id")]
    InvalidProtocolId,
    #[error("invalid version")]
    InvalidVersion,
    #[error("invalid message type {0}")]
    InvalidMessageType(u8),
    #[error("invalid varint encoding for {0}")]
    InvalidVarIntEncoding(&'static str),
    #[error("invalid name length")]
    InvalidNameLength,
    #[error("invalid name encoding")]
    InvalidNameEncoding,
}

pub(super) enum ThriftResponseMessageParser {
    Binary(BinaryMessageParser),
    Compact(CompactMessageParser),
}

impl ThriftResponseMessageParser {
    pub(super) fn parse_buf(
        &self,
        buf: &[u8],
    ) -> Result<ThriftResponseMessage, ThriftResponseMessageParseError> {
        match self {
            ThriftResponseMessageParser::Binary(p) => p.parse_buf(buf),
            ThriftResponseMessageParser::Compact(p) => p.parse_buf(buf),
        }
    }
}

pub(super) struct ThriftResponseMessage {
    pub(super) method: String,
    pub(super) seq_id: i32,
    pub(super) encoded_length: usize,
}
