/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use g3_types::codec::ThriftVarInt32;

use crate::target::thrift::protocol::{ThriftResponseMessage, ThriftResponseMessageParseError};

#[derive(Default)]
pub(crate) struct CompactMessageParser {}

impl CompactMessageParser {
    pub(crate) fn parse_buf(
        &self,
        buf: &[u8],
    ) -> Result<ThriftResponseMessage, ThriftResponseMessageParseError> {
        if buf.len() < 4 {
            return Err(ThriftResponseMessageParseError::NoEnoughData);
        }

        if buf[0] != 0x82 {
            return Err(ThriftResponseMessageParseError::InvalidProtocolId);
        }

        let message_type = buf[1] >> 5;
        if message_type != 2 {
            return Err(ThriftResponseMessageParseError::InvalidMessageType(
                message_type,
            ));
        }

        let version = buf[1] & 0x1F;
        if version != 1 {
            return Err(ThriftResponseMessageParseError::InvalidVersion);
        }

        let left = &buf[2..];
        let seq_id = ThriftVarInt32::parse(left)
            .map_err(|e| ThriftResponseMessageParseError::InvalidVarIntEncoding("seq id", e))?;

        let left = &left[seq_id.encoded_len()..];
        if left.is_empty() {
            return Err(ThriftResponseMessageParseError::NoEnoughData);
        }
        let name_len = ThriftVarInt32::parse(left).map_err(|e| {
            ThriftResponseMessageParseError::InvalidVarIntEncoding("name length", e)
        })?;

        let left = &left[name_len.encoded_len()..];
        let name_len = usize::try_from(name_len.value())
            .map_err(|_| ThriftResponseMessageParseError::InvalidNameLength)?;
        if left.len() < name_len {
            return Err(ThriftResponseMessageParseError::NoEnoughData);
        }

        let name = std::str::from_utf8(&left[..name_len])
            .map_err(|_| ThriftResponseMessageParseError::InvalidNameEncoding)?;
        let data = &left[name_len..];

        Ok(ThriftResponseMessage {
            method: name.to_string(),
            seq_id: seq_id.value(),
            encoded_length: data.len(),
        })
    }
}
