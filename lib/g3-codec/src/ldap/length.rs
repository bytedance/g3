/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use thiserror::Error;

use crate::ber::{BerLength, BerLengthParseError};

#[derive(Debug, PartialEq, Eq, Error)]
pub enum LdapLengthParseError {
    #[error("need {0} bytes more data")]
    NeedMoreData(usize),
    #[error("too large value")]
    TooLargeValue,
    #[error("indefinite value")]
    IndefiniteValue,
}

impl From<BerLengthParseError> for LdapLengthParseError {
    fn from(value: BerLengthParseError) -> Self {
        match value {
            BerLengthParseError::NeedMoreData(needed) => Self::NeedMoreData(needed),
            BerLengthParseError::TooLargeValue => Self::TooLargeValue,
        }
    }
}

#[derive(Debug)]
pub struct LdapLength {
    value: u64,
    encoded_len: usize,
}

impl LdapLength {
    /// Try to parse a BER length value with LDAP constraints from the buffer
    pub fn parse(data: &[u8]) -> Result<Self, LdapLengthParseError> {
        let ber_len = BerLength::parse(data)?;
        if ber_len.indefinite() {
            return Err(LdapLengthParseError::IndefiniteValue);
        }
        Ok(LdapLength {
            value: ber_len.value(),
            encoded_len: ber_len.encoded_len(),
        })
    }

    #[inline]
    pub fn encoded_len(&self) -> usize {
        self.encoded_len
    }

    #[inline]
    pub fn value(&self) -> u64 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert!(LdapLength::parse(b"").is_err());
        assert!(LdapLength::parse(&[0x80]).is_err());

        let v = LdapLength::parse(&[0x02]).unwrap();
        assert_eq!(v.value, 2);
        assert_eq!(v.encoded_len(), 1);
    }
}
