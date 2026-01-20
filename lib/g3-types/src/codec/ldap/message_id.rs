/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use thiserror::Error;

use crate::codec::{BerInteger, BerIntegerParseError};

#[derive(Debug, PartialEq, Eq, Error)]
pub enum LdapMessageIdParseError {
    #[error("invalid integer value: {0}")]
    InvalidIntegerValue(#[from] BerIntegerParseError),
    #[error("negative value")]
    NegativeValue,
    #[error("too large value")]
    TooLargeValue,
}

#[derive(Debug)]
pub struct LdapMessageId {
    value: u32,
    encoded_len: usize,
}

impl LdapMessageId {
    /// Try to parse a BER integer value with LDAP constraints from the buffer
    pub fn parse(data: &[u8]) -> Result<Self, LdapMessageIdParseError> {
        let ber_integer = BerInteger::parse(data)?;
        if ber_integer.value() < 0 {
            return Err(LdapMessageIdParseError::NegativeValue);
        }
        let value = u32::try_from(ber_integer.value())
            .map_err(|_| LdapMessageIdParseError::TooLargeValue)?;
        Ok(LdapMessageId {
            value,
            encoded_len: ber_integer.encoded_len(),
        })
    }

    #[inline]
    pub fn encoded_len(&self) -> usize {
        self.encoded_len
    }

    #[inline]
    pub fn value(&self) -> u32 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let v = LdapMessageId::parse(&[0x02, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 2);
        assert_eq!(v.encoded_len(), 3);
        let v = LdapMessageId::parse(&[0x02, 0x81, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 2);
        assert_eq!(v.encoded_len(), 4);
        let e = LdapMessageId::parse(&[0x02, 0x01, 0x82]).unwrap_err();
        assert_eq!(e, LdapMessageIdParseError::NegativeValue);

        let v = LdapMessageId::parse(&[0x02, 0x02, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 0x0102);
        assert_eq!(v.encoded_len(), 4);
        let e = LdapMessageId::parse(&[0x02, 0x02, 0x81, 0x02]).unwrap_err();
        assert_eq!(e, LdapMessageIdParseError::NegativeValue);

        let v = LdapMessageId::parse(&[0x02, 0x03, 0x01, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 0x010102);
        assert_eq!(v.encoded_len(), 5);
        let e = LdapMessageId::parse(&[0x02, 0x03, 0x81, 0x01, 0x02]).unwrap_err();
        assert_eq!(e, LdapMessageIdParseError::NegativeValue);

        let v = LdapMessageId::parse(&[0x02, 0x04, 0x01, 0x01, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 0x01010102);
        assert_eq!(v.encoded_len(), 6);
        let e = LdapMessageId::parse(&[0x02, 0x04, 0x81, 0x01, 0x01, 0x02]).unwrap_err();
        assert_eq!(e, LdapMessageIdParseError::NegativeValue);
    }
}
