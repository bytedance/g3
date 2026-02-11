/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use crate::quic::VarInt;

use super::FrameParseError;

pub struct CryptoFrame<'a> {
    pub stream_offset: usize,
    pub data: &'a [u8],
    encoded_len: usize,
}

impl<'a> CryptoFrame<'a> {
    pub fn new(stream_offset: usize, data: &'a [u8]) -> Self {
        CryptoFrame {
            stream_offset,
            data,
            encoded_len: 0,
        }
    }

    /// Parse a Crypto Frame from a packet buffer
    pub fn parse(data: &'a [u8]) -> Result<Self, FrameParseError> {
        let Some(stream_offset) = VarInt::parse(data) else {
            return Err(FrameParseError::NotEnoughData);
        };
        let mut offset = stream_offset.encoded_len();
        let stream_offset = usize::try_from(stream_offset.value())
            .map_err(|_| FrameParseError::TooBigOffsetValue(stream_offset.value()))?;

        let left = &data[offset..];
        let Some(length) = VarInt::parse(left) else {
            return Err(FrameParseError::NotEnoughData);
        };
        offset += length.encoded_len();

        if offset as u64 + length.value() > data.len() as u64 {
            return Err(FrameParseError::NotEnoughData);
        }

        let data_end = offset + length.value() as usize;
        Ok(CryptoFrame {
            stream_offset,
            data: &data[offset..data_end],
            encoded_len: data_end,
        })
    }

    #[inline]
    pub fn encoded_len(&self) -> usize {
        self.encoded_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn crypto_frame() {
        let data = hex!(
            "0040f1010000ed0303ebf8fa56f129 39b9584a3896472ec40bb863cfd3e868
            04fe3a47f06a2b69484c000004130113 02010000c000000010000e00000b6578
            616d706c652e636f6dff01000100000a 00080006001d00170018001000070005
            04616c706e0005000501000000000033 00260024001d00209370b2c9caa47fba
            baf4559fedba753de171fa71f50f1ce1 5d43e994ec74d748002b000302030400
            0d0010000e0403050306030203080408 050806002d00020101001c0002400100
            3900320408ffffffffffffffff050480 00ffff07048000ffff08011001048000
            75300901100f088394c8f03e51570806 048000ffff"
        );
        let frame = CryptoFrame::parse(&data).unwrap();
        assert_eq!(frame.stream_offset, 0);
        assert_eq!(frame.data.len(), data.len() - 3);
    }
}
