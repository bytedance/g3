/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

#[derive(Debug)]
pub struct QuicVarInt {
    value: u64,
    encoded_len: usize,
}

impl QuicVarInt {
    /// Try to parse a QUIC variant-length int value from the buffer
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        let value0 = data[0] & 0b0011_1111;
        match data[0] >> 6 {
            0 => Some(QuicVarInt {
                value: value0 as u64,
                encoded_len: 1,
            }),
            1 => {
                if data.len() < 2 {
                    return None;
                }
                Some(QuicVarInt {
                    value: u16::from_be_bytes([value0, data[1]]) as u64,
                    encoded_len: 2,
                })
            }
            2 => {
                if data.len() < 4 {
                    return None;
                }
                Some(QuicVarInt {
                    value: u32::from_be_bytes([value0, data[1], data[2], data[3]]) as u64,
                    encoded_len: 4,
                })
            }
            3 => {
                if data.len() < 8 {
                    return None;
                }
                Some(QuicVarInt {
                    value: u64::from_be_bytes([
                        value0, data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                    ]),
                    encoded_len: 8,
                })
            }
            _ => unreachable!(),
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
        assert!(QuicVarInt::parse(b"").is_none());

        let v = QuicVarInt::parse(&[0x02]).unwrap();
        assert_eq!(v.value, 2);
        assert_eq!(v.encoded_len(), 1);

        assert!(QuicVarInt::parse(&[0b0100_1111]).is_none());
        let v = QuicVarInt::parse(&[0b0100_1111, 0]).unwrap();
        assert_eq!(v.value, 0x0F00);
        assert_eq!(v.encoded_len(), 2);

        assert!(QuicVarInt::parse(&[0b1000_1111, 0x00]).is_none());
        let v = QuicVarInt::parse(&[0b1000_1111, 0, 0, 0x01]).unwrap();
        assert_eq!(v.value, 0x0F000001);
        assert_eq!(v.encoded_len(), 4);

        assert!(QuicVarInt::parse(&[0b1100_1111, 0]).is_none());
        let v = QuicVarInt::parse(&[0b1100_1111, 0, 0, 0, 0, 0, 0, 0x01]).unwrap();
        assert_eq!(v.value, 0x0F00000000000001);
        assert_eq!(v.encoded_len(), 8);
    }
}
