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

use super::FrameParseError;
use crate::parser::quic::VarInt;

pub struct AckFrame {
    pub largest_ack: VarInt,
    pub ack_delay: VarInt,
    pub first_ack_range: VarInt,
    pub ack_ranges: Vec<AckRange>,
    pub ecn_counts: Option<EcnCounts>,
    pub(crate) encoded_len: usize,
}

impl AckFrame {
    /// Parse an ACK Frame from a packet buffer
    pub fn parse(data: &[u8], ecn: bool) -> Result<Self, FrameParseError> {
        let mut offset = 0;

        macro_rules! read_var_int {
            ($var:ident) => {
                let Some($var) = VarInt::try_parse(&data[offset..]) else {
                    return Err(FrameParseError::NotEnoughData);
                };
                offset += $var.encoded_len();
            };
        }

        read_var_int!(largest_ack);
        read_var_int!(ack_delay);
        read_var_int!(ack_range_count);
        read_var_int!(first_ack_range);

        // the value is not trusted, so just alloc a small space
        let initial_capacity = ack_range_count.value().min(16) as usize;
        let mut ack_ranges = Vec::with_capacity(initial_capacity);
        for _ in 0..ack_range_count.value() {
            let ack_range = AckRange::parse(&data[offset..])?;
            offset += ack_range.encoded_len;
            ack_ranges.push(ack_range);
        }

        if ecn {
            let ecn_counts = EcnCounts::parse(&data[offset..])?;
            let encoded_len = offset + ecn_counts.encoded_len;
            Ok(AckFrame {
                largest_ack,
                ack_delay,
                first_ack_range,
                ack_ranges,
                ecn_counts: Some(ecn_counts),
                encoded_len,
            })
        } else {
            Ok(AckFrame {
                largest_ack,
                ack_delay,
                first_ack_range,
                ack_ranges,
                ecn_counts: None,
                encoded_len: offset,
            })
        }
    }
}

pub struct AckRange {
    pub gap: VarInt,
    pub length: VarInt,
    encoded_len: usize,
}

impl AckRange {
    fn parse(data: &[u8]) -> Result<Self, FrameParseError> {
        let Some(gap) = VarInt::try_parse(data) else {
            return Err(FrameParseError::NotEnoughData);
        };

        let offset = gap.encoded_len();
        let Some(length) = VarInt::try_parse(&data[offset..]) else {
            return Err(FrameParseError::NotEnoughData);
        };

        let encoded_len = offset + length.encoded_len();
        Ok(AckRange {
            gap,
            length,
            encoded_len,
        })
    }
}

pub struct EcnCounts {
    pub ect0: VarInt,
    pub ect1: VarInt,
    pub ecn_ce: VarInt,
    encoded_len: usize,
}

impl EcnCounts {
    fn parse(data: &[u8]) -> Result<Self, FrameParseError> {
        let mut offset = 0;

        macro_rules! read_var_int {
            ($var:ident) => {
                let Some($var) = VarInt::try_parse(&data[offset..]) else {
                    return Err(FrameParseError::NotEnoughData);
                };
                offset += $var.encoded_len();
            };
        }

        read_var_int!(ect0);
        read_var_int!(ect1);
        read_var_int!(ecn_ce);

        Ok(EcnCounts {
            ect0,
            ect1,
            ecn_ce,
            encoded_len: offset,
        })
    }
}
