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
    pub fn encode_i32(&mut self, v: i32) -> &[u8] {
        let uv = v.to_zig_zag();
        self.leb128.encode_u32(uv)
    }
}
