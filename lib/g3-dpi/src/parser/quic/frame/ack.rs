/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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

        // the value is not trusted, and each ACK Range takes at least 2 bytes,
        // so reject counts that the left buffer can never satisfy,
        // then alloc just a small space
        let ack_range_count = ack_range_count.value();
        if ack_range_count > ((data.len() - offset) / 2) as u64 {
            return Err(FrameParseError::NotEnoughData);
        }

        let initial_capacity = ack_range_count.min(16) as usize;
        let mut ack_ranges = Vec::with_capacity(initial_capacity);
        for _ in 0..ack_range_count {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ack_frame() {
        // largest_ack, ack_delay, ack_range_count, first_ack_range, [gap, length]
        let data = [0x0A, 0x00, 0x01, 0x02, 0x01, 0x03];
        let frame = AckFrame::parse(&data, false).unwrap();
        assert_eq!(frame.largest_ack.value(), 10);
        assert_eq!(frame.ack_ranges.len(), 1);
        assert_eq!(frame.encoded_len, data.len());
    }

    #[test]
    fn ack_frame_too_many_ranges() {
        // ack_range_count is 0x00FFFFFF, but only 2 bytes are left
        let data = [0x0A, 0x00, 0x80, 0xFF, 0xFF, 0xFF, 0x02, 0x01, 0x03];
        assert!(matches!(
            AckFrame::parse(&data, false),
            Err(FrameParseError::NotEnoughData)
        ));
    }
}
