/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

#[derive(Debug, PartialEq, Eq)]
pub enum LdapMessageIdParseError {
    NeedMoreData(usize),
    InvalidBerType,
    InvalidBytes,
    TooLargeValue,
    NegativeValue,
}

#[derive(Debug)]
pub struct LdapMessageId {
    value: u32,
    encoded_len: usize,
}

impl LdapMessageId {
    /// Try to parse a BER integer value with LDAP constraints from the buffer
    pub fn parse(data: &[u8]) -> Result<Self, LdapMessageIdParseError> {
        if data.len() < 3 {
            return Err(LdapMessageIdParseError::NeedMoreData(3 - data.len()));
        }

        if data[0] != 0x02 {
            return Err(LdapMessageIdParseError::InvalidBerType);
        }

        match data[1] {
            1 => {
                if data[2] & 0x80 != 0 {
                    return Err(LdapMessageIdParseError::NegativeValue);
                }

                Ok(LdapMessageId {
                    value: data[2] as u32,
                    encoded_len: 3,
                })
            }
            2 => {
                if data[2] & 0x80 != 0 {
                    return Err(LdapMessageIdParseError::NegativeValue);
                }

                if data.len() < 4 {
                    return Err(LdapMessageIdParseError::NeedMoreData(4 - data.len()));
                }

                let value = u16::from_be_bytes([data[2], data[3]]) as u32;
                Ok(LdapMessageId {
                    value,
                    encoded_len: 4,
                })
            }
            3 => {
                if data[2] & 0x80 != 0 {
                    return Err(LdapMessageIdParseError::NegativeValue);
                }

                if data.len() < 5 {
                    return Err(LdapMessageIdParseError::NeedMoreData(5 - data.len()));
                }

                let value = u32::from_be_bytes([0, data[2], data[3], data[4]]);
                Ok(LdapMessageId {
                    value,
                    encoded_len: 5,
                })
            }
            4 => {
                if data[2] & 0x80 != 0 {
                    return Err(LdapMessageIdParseError::NegativeValue);
                }

                if data.len() < 6 {
                    return Err(LdapMessageIdParseError::NeedMoreData(6 - data.len()));
                }

                let value = u32::from_be_bytes([data[2], data[3], data[4], data[5]]);
                Ok(LdapMessageId {
                    value,
                    encoded_len: 6,
                })
            }
            _ => Err(LdapMessageIdParseError::InvalidBytes),
        }
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
        let e = LdapMessageId::parse(&[0x02]).unwrap_err();
        assert_eq!(e, LdapMessageIdParseError::NeedMoreData(2));

        let e = LdapMessageId::parse(&[0x03, 0x01, 0x02]).unwrap_err();
        assert_eq!(e, LdapMessageIdParseError::InvalidBerType);
        let e = LdapMessageId::parse(&[0x02, 0x00, 0x02]).unwrap_err();
        assert_eq!(e, LdapMessageIdParseError::InvalidBytes);

        let v = LdapMessageId::parse(&[0x02, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 2);
        assert_eq!(v.encoded_len(), 3);
        let e = LdapMessageId::parse(&[0x02, 0x01, 0x82]).unwrap_err();
        assert_eq!(e, LdapMessageIdParseError::NegativeValue);

        let v = LdapMessageId::parse(&[0x02, 0x02, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 0x0102);
        assert_eq!(v.encoded_len(), 4);
        let e = LdapMessageId::parse(&[0x02, 0x02, 0x01]).unwrap_err();
        assert_eq!(e, LdapMessageIdParseError::NeedMoreData(1));
        let e = LdapMessageId::parse(&[0x02, 0x02, 0x81, 0x02]).unwrap_err();
        assert_eq!(e, LdapMessageIdParseError::NegativeValue);

        let v = LdapMessageId::parse(&[0x02, 0x03, 0x01, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 0x010102);
        assert_eq!(v.encoded_len(), 5);
        let e = LdapMessageId::parse(&[0x02, 0x03, 0x01]).unwrap_err();
        assert_eq!(e, LdapMessageIdParseError::NeedMoreData(2));
        let e = LdapMessageId::parse(&[0x02, 0x03, 0x81, 0x01, 0x02]).unwrap_err();
        assert_eq!(e, LdapMessageIdParseError::NegativeValue);

        let v = LdapMessageId::parse(&[0x02, 0x04, 0x01, 0x01, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 0x01010102);
        assert_eq!(v.encoded_len(), 6);
        let e = LdapMessageId::parse(&[0x02, 0x04, 0x01, 0x01]).unwrap_err();
        assert_eq!(e, LdapMessageIdParseError::NeedMoreData(2));
        let e = LdapMessageId::parse(&[0x02, 0x04, 0x81, 0x01, 0x01, 0x02]).unwrap_err();
        assert_eq!(e, LdapMessageIdParseError::NegativeValue);
    }
}
