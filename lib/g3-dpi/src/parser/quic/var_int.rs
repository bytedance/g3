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

use thiserror::Error;

#[derive(Debug, Error)]
pub enum VarIntParseError {
    #[error("need {0} byte(s) more data")]
    NeedMoreData(usize),
}

pub enum VarInt {
    L1(u8),
    L2(u16),
    L4(u32),
    L8(u64),
}

impl VarInt {
    pub fn parse(data: &[u8]) -> Result<Self, VarIntParseError> {
        if data.is_empty() {
            return Err(VarIntParseError::NeedMoreData(1));
        }

        let value0 = data[0] & 0b0011_1111;
        match data[0] >> 6 {
            0 => Ok(VarInt::L1(value0)),
            1 => {
                if data.len() < 2 {
                    return Err(VarIntParseError::NeedMoreData(2 - data.len()));
                }
                Ok(VarInt::L2(u16::from_be_bytes([value0, data[1]])))
            }
            2 => {
                if data.len() < 4 {
                    return Err(VarIntParseError::NeedMoreData(4 - data.len()));
                }
                Ok(VarInt::L4(u32::from_be_bytes([
                    value0, data[1], data[2], data[3],
                ])))
            }
            3 => {
                if data.len() < 8 {
                    return Err(VarIntParseError::NeedMoreData(8 - data.len()));
                }
                Ok(VarInt::L8(u64::from_be_bytes([
                    value0, data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ])))
            }
            _ => unreachable!(),
        }
    }

    pub fn encoded_len(&self) -> usize {
        match self {
            VarInt::L1(_) => 1,
            VarInt::L2(_) => 2,
            VarInt::L4(_) => 4,
            VarInt::L8(_) => 8,
        }
    }

    pub fn value(&self) -> u64 {
        match self {
            VarInt::L1(v) => *v as u64,
            VarInt::L2(v) => *v as u64,
            VarInt::L4(v) => *v as u64,
            VarInt::L8(v) => *v,
        }
    }
}
