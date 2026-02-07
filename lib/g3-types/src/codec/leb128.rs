/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Leb128DecodeError {
    #[error("need more data")]
    NeedMoreData,
    #[error("no ending byte found")]
    NoEndFound,
}

pub struct Leb128<T> {
    value: T,
    encoded_len: usize,
}

impl<T> Leb128<T> {
    pub fn encoded_len(&self) -> usize {
        self.encoded_len
    }
}

impl<T: Copy> Leb128<T> {
    pub fn value(&self) -> T {
        self.value
    }
}

impl Leb128<u32> {
    pub fn decode(data: &[u8]) -> Result<Self, Leb128DecodeError> {
        if data.is_empty() {
            return Err(Leb128DecodeError::NeedMoreData);
        }

        let bv = data[0] & 0x7F;
        if data[0] & 0x80 == 0 {
            return Ok(Leb128 {
                value: bv as u32,
                encoded_len: 1,
            });
        }

        let mut value = bv as u32;
        let mut encoded_len = 1;
        let mut total_bits = 7;
        let left = &data[1..];
        for b in left {
            encoded_len += 1;

            let bv = *b & 0x7f;
            value |= (bv as u32) << total_bits;
            if (*b & 0x80) == 0 {
                // 5 * 7 = 32, so no need to check bits for the last byte
                return Ok(Leb128 { value, encoded_len });
            } else {
                total_bits += 7;
                if total_bits > 32 {
                    return Err(Leb128DecodeError::NoEndFound);
                }
            }
        }

        Err(Leb128DecodeError::NeedMoreData)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Leb128Encoder {
    data: [u8; 10],
}

impl Leb128Encoder {
    pub fn encode_u32(&mut self, mut data: u32) -> &[u8] {
        let mut offset = 0;
        loop {
            let bv = (data & 0x7f) as u8;
            data >>= 7;
            if data == 0 {
                self.data[offset] = bv;
                return &self.data[0..=offset];
            } else {
                self.data[offset] = bv | 0x80;
                offset += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_u32() {
        let v = Leb128::<u32>::decode(&[0x01]).unwrap();
        assert_eq!(v.value, 1);
        let v = Leb128::<u32>::decode(&[0xE5, 0x8E, 0x26]).unwrap();
        assert_eq!(v.value, 624485);
    }

    #[test]
    fn encode_u32() {
        let mut encoder = Leb128Encoder::default();
        assert_eq!(encoder.encode_u32(1), &[0x01]);
        assert_eq!(encoder.encode_u32(624485), &[0xE5, 0x8E, 0x26]);
    }
}
