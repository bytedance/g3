/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use integer_encoding::VarInt;

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
        let Some((seq_id, nr)) = i32::decode_var(left) else {
            return Err(ThriftResponseMessageParseError::InvalidVarIntEncoding(
                "seq id",
            ));
        };
        if nr == 0 {
            return Err(ThriftResponseMessageParseError::InvalidVarIntEncoding(
                "seq id",
            ));
        }

        let left = &left[nr..];
        if left.is_empty() {
            return Err(ThriftResponseMessageParseError::NoEnoughData);
        }
        let Some((name_len, nr)) = i32::decode_var(left) else {
            return Err(ThriftResponseMessageParseError::InvalidVarIntEncoding(
                "name length",
            ));
        };
        if nr == 0 {
            return Err(ThriftResponseMessageParseError::InvalidVarIntEncoding(
                "name length",
            ));
        }

        let name_len = usize::try_from(name_len)
            .map_err(|_| ThriftResponseMessageParseError::InvalidNameLength)?;
        if left.len() < name_len {
            return Err(ThriftResponseMessageParseError::NoEnoughData);
        }

        let name = std::str::from_utf8(&left[..name_len])
            .map_err(|_| ThriftResponseMessageParseError::InvalidNameEncoding)?;
        let data = &left[name_len..];

        Ok(ThriftResponseMessage {
            method: name.to_string(),
            seq_id,
            encoded_length: data.len(),
        })
    }
}
