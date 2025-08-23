/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

pub struct VarInt {
    value: u64,
    encoded_len: usize,
}

impl VarInt {
    /// Try to parse a variant-length int value from the buffer
    pub fn try_parse(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        let value0 = data[0] & 0b0011_1111;
        match data[0] >> 6 {
            0 => Some(VarInt {
                value: value0 as u64,
                encoded_len: 1,
            }),
            1 => {
                if data.len() < 2 {
                    return None;
                }
                Some(VarInt {
                    value: u16::from_be_bytes([value0, data[1]]) as u64,
                    encoded_len: 2,
                })
            }
            2 => {
                if data.len() < 4 {
                    return None;
                }
                Some(VarInt {
                    value: u32::from_be_bytes([value0, data[1], data[2], data[3]]) as u64,
                    encoded_len: 4,
                })
            }
            3 => {
                if data.len() < 8 {
                    return None;
                }
                Some(VarInt {
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
