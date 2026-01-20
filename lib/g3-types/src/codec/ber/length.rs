/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

#[derive(Debug, PartialEq, Eq)]
pub enum BerLengthParseError {
    NeedMoreData(usize),
    TooLargeValue,
}

#[derive(Debug)]
pub struct BerLength {
    value: u64,
    indefinite: bool,
    encoded_len: usize,
}

impl BerLength {
    /// Try to parse a BER length value with LDAP constraints from the buffer
    pub fn parse(data: &[u8]) -> Result<Self, BerLengthParseError> {
        if data.is_empty() {
            return Err(BerLengthParseError::NeedMoreData(1));
        }

        if data[0] & 0x80 == 0 {
            return Ok(BerLength {
                value: data[0] as u64,
                indefinite: false,
                encoded_len: 1,
            });
        }

        match data[0] & 0x7F {
            0 => Ok(BerLength {
                value: 0,
                indefinite: true,
                encoded_len: 1,
            }),
            1 => {
                if data.len() < 2 {
                    return Err(BerLengthParseError::NeedMoreData(2 - data.len()));
                }
                Ok(BerLength {
                    value: data[1] as u64,
                    indefinite: false,
                    encoded_len: 2,
                })
            }
            2 => {
                if data.len() < 3 {
                    return Err(BerLengthParseError::NeedMoreData(3 - data.len()));
                }
                Ok(BerLength {
                    value: u16::from_be_bytes([data[1], data[2]]) as u64,
                    indefinite: false,
                    encoded_len: 3,
                })
            }
            3 => {
                if data.len() < 4 {
                    return Err(BerLengthParseError::NeedMoreData(4 - data.len()));
                }
                Ok(BerLength {
                    value: u32::from_be_bytes([0, data[1], data[2], data[3]]) as u64,
                    indefinite: false,
                    encoded_len: 4,
                })
            }
            4 => {
                if data.len() < 5 {
                    return Err(BerLengthParseError::NeedMoreData(5 - data.len()));
                }
                Ok(BerLength {
                    value: u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as u64,
                    indefinite: false,
                    encoded_len: 5,
                })
            }
            5 => {
                if data.len() < 6 {
                    return Err(BerLengthParseError::NeedMoreData(6 - data.len()));
                }
                Ok(BerLength {
                    value: u64::from_be_bytes([
                        0, 0, 0, data[1], data[2], data[3], data[4], data[5],
                    ]),
                    indefinite: false,
                    encoded_len: 6,
                })
            }
            6 => {
                if data.len() < 7 {
                    return Err(BerLengthParseError::NeedMoreData(7 - data.len()));
                }
                Ok(BerLength {
                    value: u64::from_be_bytes([
                        0, 0, data[1], data[2], data[3], data[4], data[5], data[6],
                    ]),
                    indefinite: false,
                    encoded_len: 7,
                })
            }
            7 => {
                if data.len() < 8 {
                    return Err(BerLengthParseError::NeedMoreData(8 - data.len()));
                }
                Ok(BerLength {
                    value: u64::from_be_bytes([
                        0, data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                    ]),
                    indefinite: false,
                    encoded_len: 8,
                })
            }
            8 => {
                if data.len() < 9 {
                    return Err(BerLengthParseError::NeedMoreData(9 - data.len()));
                }
                Ok(BerLength {
                    value: u64::from_be_bytes([
                        data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8],
                    ]),
                    indefinite: false,
                    encoded_len: 9,
                })
            }
            _ => Err(BerLengthParseError::TooLargeValue),
        }
    }

    #[inline]
    pub fn indefinite(&self) -> bool {
        self.indefinite
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

#[derive(Default)]
pub struct BerLengthEncoder {
    buf: [u8; 9],
    offset: usize,
}

impl BerLengthEncoder {
    pub fn encode(&mut self, value: usize) -> &[u8] {
        if value <= 0x7F {
            self.offset = 8;
            self.buf[8] = (value & 0x7F) as u8;
            return &self.buf[8..];
        }

        let bytes = (value as u64).to_be_bytes();
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.buf[1..].as_mut_ptr(), 8);
        }
        let unused_bits = value.leading_zeros() as usize;
        self.offset = unused_bits / 8;
        self.buf[self.offset] = 8 - self.offset as u8;
        &self.buf[self.offset..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert!(BerLength::parse(b"").is_err());

        let v = BerLength::parse(&[0x80]).unwrap();
        assert_eq!(v.encoded_len(), 1);
        assert!(v.indefinite());

        let v = BerLength::parse(&[0x02]).unwrap();
        assert_eq!(v.value, 2);
        assert_eq!(v.encoded_len(), 1);

        let v = BerLength::parse(&[0x81]).unwrap_err();
        assert_eq!(v, BerLengthParseError::NeedMoreData(1));
        let v = BerLength::parse(&[0x81, 0x01]).unwrap();
        assert_eq!(v.value, 0x01);
        assert_eq!(v.encoded_len(), 2);

        let v = BerLength::parse(&[0x82, 0]).unwrap_err();
        assert_eq!(v, BerLengthParseError::NeedMoreData(1));
        let v = BerLength::parse(&[0x82, 0, 0x01]).unwrap();
        assert_eq!(v.value, 0x01);
        assert_eq!(v.encoded_len(), 3);

        let v = BerLength::parse(&[0x83, 0]).unwrap_err();
        assert_eq!(v, BerLengthParseError::NeedMoreData(2));
        let v = BerLength::parse(&[0x83, 0, 0, 0x01]).unwrap();
        assert_eq!(v.value, 0x01);
        assert_eq!(v.encoded_len(), 4);

        let v = BerLength::parse(&[0x84, 0, 0]).unwrap_err();
        assert_eq!(v, BerLengthParseError::NeedMoreData(2));
        let v = BerLength::parse(&[0x84, 0, 0, 0, 0x01]).unwrap();
        assert_eq!(v.value, 0x01);
        assert_eq!(v.encoded_len(), 5);

        let v = BerLength::parse(&[0x85, 0, 0]).unwrap_err();
        assert_eq!(v, BerLengthParseError::NeedMoreData(3));
        let v = BerLength::parse(&[0x85, 0, 0, 0, 0, 0x01]).unwrap();
        assert_eq!(v.value, 0x01);
        assert_eq!(v.encoded_len(), 6);

        let v = BerLength::parse(&[0x86, 0, 0, 0]).unwrap_err();
        assert_eq!(v, BerLengthParseError::NeedMoreData(3));
        let v = BerLength::parse(&[0x86, 0, 0, 0, 0, 0, 0x01]).unwrap();
        assert_eq!(v.value, 0x01);
        assert_eq!(v.encoded_len(), 7);

        let v = BerLength::parse(&[0x87, 0, 0, 0]).unwrap_err();
        assert_eq!(v, BerLengthParseError::NeedMoreData(4));
        let v = BerLength::parse(&[0x87, 0, 0, 0, 0, 0, 0, 0x01]).unwrap();
        assert_eq!(v.value, 0x01);
        assert_eq!(v.encoded_len(), 8);

        let v = BerLength::parse(&[0x88, 0, 0, 0]).unwrap_err();
        assert_eq!(v, BerLengthParseError::NeedMoreData(5));
        let v = BerLength::parse(&[0x88, 0, 0, 0, 0, 0, 0, 0, 0x01]).unwrap();
        assert_eq!(v.value, 0x01);
        assert_eq!(v.encoded_len(), 9);

        let v = BerLength::parse(&[0x89, 0, 0, 0]).unwrap_err();
        assert_eq!(v, BerLengthParseError::TooLargeValue);
    }

    #[test]
    fn encode_32() {
        let mut encoder = BerLengthEncoder::default();
        assert_eq!(encoder.encode(0), &[0]);
        assert_eq!(encoder.encode(1), &[1]);
        assert_eq!(encoder.encode(0x7F), &[0x7F]);
        assert_eq!(encoder.encode(0x80), &[0x01, 0x80]);
        assert_eq!(encoder.encode(0x0100), &[0x02, 0x01, 0x00]);
        assert_eq!(encoder.encode(0x010000), &[0x03, 0x01, 0x00, 0x00]);
        assert_eq!(encoder.encode(0x01000000), &[0x04, 0x01, 0x00, 0x00, 0x00]);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn encode_64() {
        let mut encoder = BerLengthEncoder::default();
        assert_eq!(
            encoder.encode(0x0100000000),
            &[0x05, 0x01, 0x00, 0x00, 0x00, 0x00]
        );
        assert_eq!(
            encoder.encode(0x010000000000),
            &[0x06, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00]
        );
        assert_eq!(
            encoder.encode(0x01000000000000),
            &[0x07, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        );
        assert_eq!(
            encoder.encode(0x0100000000000000),
            &[0x08, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        );
    }
}
