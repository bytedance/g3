/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use thiserror::Error;

use crate::ber::{BerLength, BerLengthParseError};

#[derive(Debug, Error)]
pub enum LdapSequenceParseError {
    #[error("need {0} bytes more data")]
    NeedMoreData(usize),
    #[error("invalid ber type")]
    InvalidType,
    #[error("invalid ber length")]
    TooLargeLength,
    #[error("indefinite length")]
    IndefiniteLength,
}

impl From<BerLengthParseError> for LdapSequenceParseError {
    fn from(value: BerLengthParseError) -> Self {
        match value {
            BerLengthParseError::NeedMoreData(n) => LdapSequenceParseError::NeedMoreData(n),
            BerLengthParseError::TooLargeValue => LdapSequenceParseError::TooLargeLength,
        }
    }
}

pub struct LdapSequence<'a> {
    data: &'a [u8],
    encoded_len: usize,
}

impl<'a> LdapSequence<'a> {
    pub fn parse_octet_string(data: &'a [u8]) -> Result<Self, LdapSequenceParseError> {
        Self::parse_with_identifier(data, 0x04)
    }

    pub fn parse_referrals_sequence(data: &'a [u8]) -> Result<Self, LdapSequenceParseError> {
        Self::parse_with_identifier(data, 0xa3)
    }

    pub fn parse_bind_response(data: &'a [u8]) -> Result<Self, LdapSequenceParseError> {
        Self::parse_with_identifier(data, 0x61)
    }

    pub fn parse_extended_response(data: &'a [u8]) -> Result<Self, LdapSequenceParseError> {
        Self::parse_with_identifier(data, 0x78)
    }

    pub fn parse_extended_response_oid(data: &'a [u8]) -> Result<Self, LdapSequenceParseError> {
        Self::parse_with_identifier(data, 0x8a)
    }

    fn parse_with_identifier(
        data: &'a [u8],
        identifier: u8,
    ) -> Result<Self, LdapSequenceParseError> {
        if data.is_empty() {
            return Err(LdapSequenceParseError::NeedMoreData(1));
        }
        if data[0] != identifier {
            return Err(LdapSequenceParseError::InvalidType);
        }

        let ber_length = BerLength::parse(&data[1..])?;
        if ber_length.indefinite() {
            return Err(LdapSequenceParseError::IndefiniteLength);
        }

        let offset = 1 + ber_length.encoded_len();
        if ber_length.value() == 0 {
            Ok(LdapSequence {
                data: b"",
                encoded_len: offset,
            })
        } else if ber_length.value() + offset as u64 > data.len() as u64 {
            Err(LdapSequenceParseError::TooLargeLength)
        } else {
            let encoded_len = ber_length.value() as usize + offset;
            Ok(LdapSequence {
                data: &data[offset..encoded_len],
                encoded_len,
            })
        }
    }

    #[inline]
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    #[inline]
    pub fn encoded_len(&self) -> usize {
        self.encoded_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert!(LdapSequence::parse_octet_string(b"").is_err());

        let v = LdapSequence::parse_octet_string(&[0x04, 0x00]).unwrap();
        assert_eq!(v.data, b"");
        assert_eq!(v.encoded_len(), 2);

        let v = LdapSequence::parse_octet_string(&[0x04, 0x02, 0x01, 0x02]).unwrap();
        assert_eq!(v.data, &[0x01, 0x02]);
        assert_eq!(v.encoded_len(), 4);
    }
}
