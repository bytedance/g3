/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use g3_std_ext::core::{FromZigZag, ToZigZag};

use crate::codec::{Leb128, Leb128DecodeError, Leb128Encoder};

pub struct ThriftVarInt32 {
    leb128: Leb128<u32>,
}

impl ThriftVarInt32 {
    pub fn parse(data: &[u8]) -> Result<ThriftVarInt32, Leb128DecodeError> {
        let leb128 = Leb128::decode(data)?;
        Ok(ThriftVarInt32 { leb128 })
    }

    // Get the varint value used directly in thrift protocol,
    // which is always positive and not zigzag encoded
    pub fn positive_value(&self) -> i32 {
        self.leb128.value() as i32
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
pub struct ThriftVarIntEncoder {
    leb128: Leb128Encoder,
}

impl ThriftVarIntEncoder {
    // Encode the thrift integer value, with correct zigzag encoding
    pub fn encode_i32(&mut self, v: i32) -> &[u8] {
        let uv = v.to_zig_zag();
        self.leb128.encode_u32(uv)
    }

    // Encode the positive varint used directly in thrift protocol, which will not be zigzag encoded
    pub fn encode_positive_i32(&mut self, v: i32) -> &[u8] {
        self.leb128.encode_u32(v as u32)
    }
}
