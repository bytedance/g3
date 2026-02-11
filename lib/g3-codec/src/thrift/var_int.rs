/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use g3_std_ext::core::{FromZigZag, ToZigZag};

use crate::leb128::{Leb128, Leb128DecodeError, Leb128Encoder};

pub struct VarInt32 {
    leb128: Leb128<u32>,
}

impl VarInt32 {
    pub fn parse(data: &[u8]) -> Result<VarInt32, Leb128DecodeError> {
        let leb128 = Leb128::decode(data)?;
        Ok(VarInt32 { leb128 })
    }

    // Get the varint value used directly in thrift protocol,
    // which is always positive and not zigzag encoded
    pub fn positive_value(&self) -> i32 {
        u32::cast_signed(self.leb128.value())
    }

    // Get the thrift integer value, which is zigzag encoded
    pub fn value(&self) -> i32 {
        let uv = self.leb128.value();
        i32::from_zig_zag(uv)
    }

    pub fn encoded_len(&self) -> usize {
        self.leb128.encoded_len()
    }
}

#[derive(Default)]
pub struct VarIntEncoder {
    leb128: Leb128Encoder,
}

impl VarIntEncoder {
    // Encode the thrift integer value, with correct zigzag encoding
    pub fn encode_i32(&mut self, v: i32) -> &[u8] {
        let uv = v.to_zig_zag();
        self.leb128.encode_u32(uv)
    }

    // Encode the positive varint used directly in thrift protocol, which will not be zigzag encoded
    pub fn encode_positive_i32(&mut self, v: i32) -> &[u8] {
        self.leb128.encode_u32(i32::cast_unsigned(v))
    }
}
