/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use thiserror::Error;

use super::{LdapLength, LdapLengthParseError, LdapMessageId, LdapMessageIdParseError};

#[derive(Debug, Error)]
pub enum LdapMessageParseError {
    #[error("need {0} bytes more data")]
    NeedMoreData(usize),
    #[error("invalid message ber type")]
    InvalidBerType,
    #[error("invalid message length value")]
    InvalidMessageLength,
    #[error("invalid message id: {0}")]
    InvalidMessageId(#[from] LdapMessageIdParseError),
}

impl From<LdapLengthParseError> for LdapMessageParseError {
    fn from(value: LdapLengthParseError) -> Self {
        match value {
            LdapLengthParseError::NeedMoreData(n) => LdapMessageParseError::NeedMoreData(n),
            LdapLengthParseError::TooLargeValue | LdapLengthParseError::IndefiniteValue => {
                LdapMessageParseError::InvalidMessageLength
            }
        }
    }
}

pub struct LdapMessage<'a> {
    id: u32,
    payload: &'a [u8],
    encoded_size: usize,
}

impl<'a> LdapMessage<'a> {
    pub fn parse(data: &'a [u8], max_message_length: usize) -> Result<Self, LdapMessageParseError> {
        if data.is_empty() {
            return Err(LdapMessageParseError::NeedMoreData(1));
        }

        if data[0] != 0x30 {
            return Err(LdapMessageParseError::InvalidBerType);
        }
        let mut offset = 1usize;

        let length = LdapLength::parse(&data[offset..])?;
        offset += length.encoded_len();
        let message_length = length.value();
        if message_length > max_message_length as u64 {
            return Err(LdapMessageParseError::InvalidMessageLength);
        }
        let message_length = message_length as usize;

        let left = &data[offset..];
        if left.len() < message_length {
            return Err(LdapMessageParseError::NeedMoreData(
                message_length - left.len(),
            ));
        }

        let id = LdapMessageId::parse(&left[..message_length])?;
        let message_id = id.value();

        Ok(LdapMessage {
            id: message_id,
            payload: &left[id.encoded_len()..message_length],
            encoded_size: offset + message_length,
        })
    }

    #[inline]
    pub fn id(&self) -> u32 {
        self.id
    }

    #[inline]
    pub fn payload(&self) -> &'a [u8] {
        self.payload
    }

    #[inline]
    pub fn encoded_size(&self) -> usize {
        self.encoded_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_bind_response() {
        let data = [
            0x30, 0x0c, // Begin the LDAPMessage sequence
            0x02, 0x01, 0x01, // The message ID (integer value 1)
            0x61, 0x07, // Begin the bind response protocol op
            0x0a, 0x01, 0x00, // success result code (enumerated value 0)
            0x04, 0x00, // No matched DN (0-byte octet string)
            0x04, 0x00, // No diagnostic message (0-byte octet string)
        ];
        let message = LdapMessage::parse(&data, 128).unwrap();
        assert_eq!(message.id(), 1);
        assert_eq!(message.payload(), &data[5..]);
        assert_eq!(message.encoded_size(), data.len());
    }
}
