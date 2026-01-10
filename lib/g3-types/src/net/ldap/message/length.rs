/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

#[derive(Debug, PartialEq, Eq)]
pub enum LdapMessageLengthParseError {
    NeedMoreData(usize),
    TooLargeValue,
}

#[derive(Debug)]
pub struct LdapMessageLength {
    value: u64,
    encoded_len: usize,
}

impl LdapMessageLength {
    /// Try to parse a BER length value with LDAP constraints from the buffer
    pub fn parse(data: &[u8]) -> Result<Self, LdapMessageLengthParseError> {
        if data.is_empty() {
            return Err(LdapMessageLengthParseError::NeedMoreData(1));
        }

        if data[0] & 0x80 == 0 {
            return Ok(LdapMessageLength {
                value: data[0] as u64,
                encoded_len: 1,
            });
        }

        match data[0] & 0x7F {
            0 => Ok(LdapMessageLength {
                value: 0,
                encoded_len: 1,
            }),
            1 => {
                if data.len() < 2 {
                    return Err(LdapMessageLengthParseError::NeedMoreData(2 - data.len()));
                }
                Ok(LdapMessageLength {
                    value: data[1] as u64,
                    encoded_len: 2,
                })
            }
            2 => {
                if data.len() < 3 {
                    return Err(LdapMessageLengthParseError::NeedMoreData(3 - data.len()));
                }
                Ok(LdapMessageLength {
                    value: u16::from_be_bytes([data[1], data[2]]) as u64,
                    encoded_len: 3,
                })
            }
            3 => {
                if data.len() < 4 {
                    return Err(LdapMessageLengthParseError::NeedMoreData(4 - data.len()));
                }
                Ok(LdapMessageLength {
                    value: u32::from_be_bytes([0, data[1], data[2], data[3]]) as u64,
                    encoded_len: 4,
                })
            }
            4 => {
                if data.len() < 5 {
                    return Err(LdapMessageLengthParseError::NeedMoreData(5 - data.len()));
                }
                Ok(LdapMessageLength {
                    value: u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as u64,
                    encoded_len: 5,
                })
            }
            5 => {
                if data.len() < 6 {
                    return Err(LdapMessageLengthParseError::NeedMoreData(6 - data.len()));
                }
                Ok(LdapMessageLength {
                    value: u64::from_be_bytes([
                        0, 0, 0, data[1], data[2], data[3], data[4], data[5],
                    ]),
                    encoded_len: 6,
                })
            }
            6 => {
                if data.len() < 7 {
                    return Err(LdapMessageLengthParseError::NeedMoreData(7 - data.len()));
                }
                Ok(LdapMessageLength {
                    value: u64::from_be_bytes([
                        0, 0, data[1], data[2], data[3], data[4], data[5], data[6],
                    ]),
                    encoded_len: 7,
                })
            }
            7 => {
                if data.len() < 8 {
                    return Err(LdapMessageLengthParseError::NeedMoreData(8 - data.len()));
                }
                Ok(LdapMessageLength {
                    value: u64::from_be_bytes([
                        0, data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                    ]),
                    encoded_len: 8,
                })
            }
            8 => {
                if data.len() < 9 {
                    return Err(LdapMessageLengthParseError::NeedMoreData(9 - data.len()));
                }
                Ok(LdapMessageLength {
                    value: u64::from_be_bytes([
                        data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8],
                    ]),
                    encoded_len: 9,
                })
            }
            _ => Err(LdapMessageLengthParseError::TooLargeValue),
        }
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
        assert!(LdapMessageLength::parse(b"").is_err());

        let v = LdapMessageLength::parse(&[0x02]).unwrap();
        assert_eq!(v.value, 2);
        assert_eq!(v.encoded_len(), 1);

        let v = LdapMessageLength::parse(&[0x80]).unwrap();
        assert_eq!(v.value, 0);
        assert_eq!(v.encoded_len(), 1);

        let v = LdapMessageLength::parse(&[0x81]).unwrap_err();
        assert_eq!(v, LdapMessageLengthParseError::NeedMoreData(1));
        let v = LdapMessageLength::parse(&[0x81, 0x01]).unwrap();
        assert_eq!(v.value, 0x01);
        assert_eq!(v.encoded_len(), 2);

        let v = LdapMessageLength::parse(&[0x82, 0]).unwrap_err();
        assert_eq!(v, LdapMessageLengthParseError::NeedMoreData(1));
        let v = LdapMessageLength::parse(&[0x82, 0, 0x01]).unwrap();
        assert_eq!(v.value, 0x01);
        assert_eq!(v.encoded_len(), 3);

        let v = LdapMessageLength::parse(&[0x83, 0]).unwrap_err();
        assert_eq!(v, LdapMessageLengthParseError::NeedMoreData(2));
        let v = LdapMessageLength::parse(&[0x83, 0, 0, 0x01]).unwrap();
        assert_eq!(v.value, 0x01);
        assert_eq!(v.encoded_len(), 4);

        let v = LdapMessageLength::parse(&[0x84, 0, 0]).unwrap_err();
        assert_eq!(v, LdapMessageLengthParseError::NeedMoreData(2));
        let v = LdapMessageLength::parse(&[0x84, 0, 0, 0, 0x01]).unwrap();
        assert_eq!(v.value, 0x01);
        assert_eq!(v.encoded_len(), 5);

        let v = LdapMessageLength::parse(&[0x85, 0, 0]).unwrap_err();
        assert_eq!(v, LdapMessageLengthParseError::NeedMoreData(3));
        let v = LdapMessageLength::parse(&[0x85, 0, 0, 0, 0, 0x01]).unwrap();
        assert_eq!(v.value, 0x01);
        assert_eq!(v.encoded_len(), 6);

        let v = LdapMessageLength::parse(&[0x86, 0, 0, 0]).unwrap_err();
        assert_eq!(v, LdapMessageLengthParseError::NeedMoreData(3));
        let v = LdapMessageLength::parse(&[0x86, 0, 0, 0, 0, 0, 0x01]).unwrap();
        assert_eq!(v.value, 0x01);
        assert_eq!(v.encoded_len(), 7);

        let v = LdapMessageLength::parse(&[0x87, 0, 0, 0]).unwrap_err();
        assert_eq!(v, LdapMessageLengthParseError::NeedMoreData(4));
        let v = LdapMessageLength::parse(&[0x87, 0, 0, 0, 0, 0, 0, 0x01]).unwrap();
        assert_eq!(v.value, 0x01);
        assert_eq!(v.encoded_len(), 8);

        let v = LdapMessageLength::parse(&[0x88, 0, 0, 0]).unwrap_err();
        assert_eq!(v, LdapMessageLengthParseError::NeedMoreData(5));
        let v = LdapMessageLength::parse(&[0x88, 0, 0, 0, 0, 0, 0, 0, 0x01]).unwrap();
        assert_eq!(v.value, 0x01);
        assert_eq!(v.encoded_len(), 9);

        let v = LdapMessageLength::parse(&[0x89, 0, 0, 0]).unwrap_err();
        assert_eq!(v, LdapMessageLengthParseError::TooLargeValue);
    }
}
