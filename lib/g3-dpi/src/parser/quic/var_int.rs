/*
 * Copyright 2024 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

pub struct VarInt {
    value: u64,
    encoded_len: usize,
}

impl VarInt {
    pub fn parse(data: &[u8]) -> Option<Self> {
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
