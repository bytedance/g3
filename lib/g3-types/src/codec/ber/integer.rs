/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use thiserror::Error;

use super::{BerLength, BerLengthParseError};

#[derive(Debug, PartialEq, Eq, Error)]
pub enum BerIntegerParseError {
    #[error("need {0} bytes more data")]
    NeedMoreData(usize),
    #[error("invalid ber type")]
    InvalidType,
    #[error("invalid ber length")]
    TooLargeLength,
    #[error("indefinite length")]
    IndefiniteLength,
    #[error("invalid value bytes")]
    InvalidValueBytes,
}

impl From<BerLengthParseError> for BerIntegerParseError {
    fn from(value: BerLengthParseError) -> Self {
        match value {
            BerLengthParseError::NeedMoreData(n) => BerIntegerParseError::NeedMoreData(n),
            BerLengthParseError::TooLargeValue => BerIntegerParseError::TooLargeLength,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BerInteger {
    value: i64,
    encoded_len: usize,
}

impl BerInteger {
    pub fn parse(data: &[u8]) -> Result<BerInteger, BerIntegerParseError> {
        Self::parse_with_identifier(data, 0x02)
    }

    pub fn parse_enumerated_value(data: &[u8]) -> Result<BerInteger, BerIntegerParseError> {
        Self::parse_with_identifier(data, 0x0a)
    }

    fn parse_with_identifier(data: &[u8], identifier: u8) -> Result<Self, BerIntegerParseError> {
        if data.is_empty() {
            return Err(BerIntegerParseError::NeedMoreData(1));
        }
        if data[0] != identifier {
            return Err(BerIntegerParseError::InvalidType);
        }

        let length = BerLength::parse(&data[1..])?;
        if length.indefinite() {
            return Err(BerIntegerParseError::IndefiniteLength);
        }

        let offset = 1 + length.encoded_len();
        let left = &data[offset..];
        let value0 = left[0] & 0x7F;
        let mut integer = match length.value() {
            1 => {
                if left.is_empty() {
                    return Err(BerIntegerParseError::NeedMoreData(1));
                }
                BerInteger {
                    value: i64::from(value0),
                    encoded_len: offset + 1,
                }
            }
            2 => {
                if left.len() < 2 {
                    return Err(BerIntegerParseError::NeedMoreData(2 - left.len()));
                }
                BerInteger {
                    value: i16::from_be_bytes([value0, left[1]]) as i64,
                    encoded_len: offset + 2,
                }
            }
            3 => {
                if left.len() < 3 {
                    return Err(BerIntegerParseError::NeedMoreData(3 - left.len()));
                }
                BerInteger {
                    value: i32::from_be_bytes([0, value0, left[1], left[2]]) as i64,
                    encoded_len: offset + 3,
                }
            }
            4 => {
                if left.len() < 4 {
                    return Err(BerIntegerParseError::NeedMoreData(4 - left.len()));
                }
                BerInteger {
                    value: i32::from_be_bytes([value0, left[1], left[2], left[3]]) as i64,
                    encoded_len: offset + 4,
                }
            }
            5 => {
                if left.len() < 5 {
                    return Err(BerIntegerParseError::NeedMoreData(5 - left.len()));
                }
                BerInteger {
                    value: i64::from_be_bytes([
                        0, 0, 0, value0, left[1], left[2], left[3], left[4],
                    ]),
                    encoded_len: offset + 5,
                }
            }
            6 => {
                if left.len() < 6 {
                    return Err(BerIntegerParseError::NeedMoreData(6 - left.len()));
                }
                BerInteger {
                    value: i64::from_be_bytes([
                        0, 0, value0, left[1], left[2], left[3], left[4], left[5],
                    ]),
                    encoded_len: offset + 6,
                }
            }
            7 => {
                if left.len() < 7 {
                    return Err(BerIntegerParseError::NeedMoreData(7 - left.len()));
                }
                BerInteger {
                    value: i64::from_be_bytes([
                        0, value0, left[1], left[2], left[3], left[4], left[5], left[6],
                    ]),
                    encoded_len: offset + 7,
                }
            }
            8 => {
                if left.len() < 8 {
                    return Err(BerIntegerParseError::NeedMoreData(8 - left.len()));
                }
                BerInteger {
                    value: i64::from_be_bytes([
                        value0, left[1], left[2], left[3], left[4], left[5], left[6], left[7],
                    ]),
                    encoded_len: offset + 8,
                }
            }
            _ => return Err(BerIntegerParseError::InvalidValueBytes),
        };
        if left[0] >> 7 != 0 {
            integer.value = 0 - integer.value;
        }
        Ok(integer)
    }

    #[inline]
    pub fn encoded_len(&self) -> usize {
        self.encoded_len
    }

    #[inline]
    pub fn value(&self) -> i64 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let e = BerInteger::parse(&[0x02]).unwrap_err();
        assert_eq!(e, BerIntegerParseError::NeedMoreData(1));

        let e = BerInteger::parse(&[0x03, 0x01, 0x02]).unwrap_err();
        assert_eq!(e, BerIntegerParseError::InvalidType);
        let e = BerInteger::parse(&[0x02, 0x00, 0x02]).unwrap_err();
        assert_eq!(e, BerIntegerParseError::InvalidValueBytes);

        let v = BerInteger::parse(&[0x02, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 2);
        assert_eq!(v.encoded_len(), 3);
        let v = BerInteger::parse(&[0x02, 0x81, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 2);
        assert_eq!(v.encoded_len(), 4);
        let v = BerInteger::parse(&[0x02, 0x01, 0x82]).unwrap();
        assert_eq!(v.value, -2);
        assert_eq!(v.encoded_len(), 3);

        let v = BerInteger::parse(&[0x02, 0x02, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 0x0102);
        assert_eq!(v.encoded_len(), 4);
        let e = BerInteger::parse(&[0x02, 0x02, 0x01]).unwrap_err();
        assert_eq!(e, BerIntegerParseError::NeedMoreData(1));
        let v = BerInteger::parse(&[0x02, 0x02, 0x81, 0x02]).unwrap();
        assert_eq!(v.value, -0x0102);
        assert_eq!(v.encoded_len(), 4);

        let v = BerInteger::parse(&[0x02, 0x03, 0x01, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 0x010102);
        assert_eq!(v.encoded_len(), 5);
        let e = BerInteger::parse(&[0x02, 0x03, 0x01]).unwrap_err();
        assert_eq!(e, BerIntegerParseError::NeedMoreData(2));
        let v = BerInteger::parse(&[0x02, 0x03, 0x81, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, -0x010102);
        assert_eq!(v.encoded_len(), 5);

        let v = BerInteger::parse(&[0x02, 0x04, 0x01, 0x01, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 0x01010102);
        assert_eq!(v.encoded_len(), 6);
        let e = BerInteger::parse(&[0x02, 0x04, 0x01, 0x01]).unwrap_err();
        assert_eq!(e, BerIntegerParseError::NeedMoreData(2));
        let v = BerInteger::parse(&[0x02, 0x04, 0x81, 0x01, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, -0x01010102);
        assert_eq!(v.encoded_len(), 6);

        let v = BerInteger::parse(&[0x02, 0x05, 0x01, 0x01, 0x01, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 0x0101010102);
        assert_eq!(v.encoded_len(), 7);
        let e = BerInteger::parse(&[0x02, 0x05, 0x01, 0x01]).unwrap_err();
        assert_eq!(e, BerIntegerParseError::NeedMoreData(3));
        let v = BerInteger::parse(&[0x02, 0x05, 0x81, 0x01, 0x01, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, -0x0101010102);
        assert_eq!(v.encoded_len(), 7);

        let v = BerInteger::parse(&[0x02, 0x06, 0, 0x01, 0x01, 0x01, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 0x0101010102);
        assert_eq!(v.encoded_len(), 8);
        let e = BerInteger::parse(&[0x02, 0x06, 0x01, 0x01]).unwrap_err();
        assert_eq!(e, BerIntegerParseError::NeedMoreData(4));
        let v = BerInteger::parse(&[0x02, 0x06, 0x80, 0x01, 0x01, 0x01, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, -0x0101010102);
        assert_eq!(v.encoded_len(), 8);

        let v = BerInteger::parse(&[0x02, 0x07, 0, 0, 0x01, 0x01, 0x01, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 0x0101010102);
        assert_eq!(v.encoded_len(), 9);
        let e = BerInteger::parse(&[0x02, 0x07, 0x01, 0x01]).unwrap_err();
        assert_eq!(e, BerIntegerParseError::NeedMoreData(5));
        let v = BerInteger::parse(&[0x02, 0x07, 0x80, 0, 0x01, 0x01, 0x01, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, -0x0101010102);
        assert_eq!(v.encoded_len(), 9);

        let v = BerInteger::parse(&[0x02, 0x08, 0, 0, 0, 0x01, 0x01, 0x01, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, 0x0101010102);
        assert_eq!(v.encoded_len(), 10);
        let e = BerInteger::parse(&[0x02, 0x08, 0x01, 0x01]).unwrap_err();
        assert_eq!(e, BerIntegerParseError::NeedMoreData(6));
        let v = BerInteger::parse(&[0x02, 0x08, 0x80, 0, 0, 0x01, 0x01, 0x01, 0x01, 0x02]).unwrap();
        assert_eq!(v.value, -0x0101010102);
        assert_eq!(v.encoded_len(), 10);
    }
}
