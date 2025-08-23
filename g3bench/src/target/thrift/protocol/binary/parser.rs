/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use crate::target::thrift::protocol::{ThriftResponseMessage, ThriftResponseMessageParseError};

#[derive(Default)]
pub(crate) struct BinaryMessageParser {}

impl BinaryMessageParser {
    pub(crate) fn parse_buf(
        &self,
        buf: &[u8],
    ) -> Result<ThriftResponseMessage, ThriftResponseMessageParseError> {
        if buf.len() < 2 + 2 + 4 {
            return Err(ThriftResponseMessageParseError::NoEnoughData);
        }

        if buf[0] != 0x80 || buf[1] != 0x01 {
            return Err(ThriftResponseMessageParseError::InvalidVersion);
        }

        let message_type = buf[3] & 0x07;
        if message_type != 2 {
            return Err(ThriftResponseMessageParseError::InvalidMessageType(
                message_type,
            ));
        }

        let name_len = i32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let name_len = usize::try_from(name_len)
            .map_err(|_| ThriftResponseMessageParseError::InvalidNameLength)?;

        let left = &buf[8..];
        if left.len() < name_len + 4 {
            return Err(ThriftResponseMessageParseError::NoEnoughData);
        }

        let name = std::str::from_utf8(&left[..name_len])
            .map_err(|_| ThriftResponseMessageParseError::InvalidNameEncoding)?;
        let left = &left[name_len..];
        let seq_id = i32::from_be_bytes([left[0], left[1], left[2], left[3]]);
        let data = &left[4..];

        Ok(ThriftResponseMessage {
            method: name.to_string(),
            seq_id,
            encoded_length: data.len(),
        })
    }
}
