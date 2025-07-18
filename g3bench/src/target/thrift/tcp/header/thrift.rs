/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeMap;

use anyhow::{Context, anyhow};
use integer_encoding::VarInt;

use super::HeaderBufOffsets;
use crate::target::thrift::protocol::ThriftProtocol;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct StringValue {
    len_bytes: Vec<u8>,
    value: String,
}

impl TryFrom<String> for StringValue {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let Ok(len) = i16::try_from(value.len()) else {
            return Err(anyhow!("too long string length"));
        };

        Ok(StringValue {
            len_bytes: len.encode_var_vec(),
            value,
        })
    }
}

#[derive(Default)]
pub(crate) struct ThriftTHeaderBuilder {
    info_key_values: BTreeMap<StringValue, StringValue>,
}

impl ThriftTHeaderBuilder {
    pub(crate) fn add_info_kv(&mut self, k: &str, v: &str) -> anyhow::Result<()> {
        if k.is_empty() {
            return Err(anyhow!("empty key"));
        }
        let k = StringValue::try_from(k.to_string()).context(format!("invalid key: {k}"))?;
        let v = StringValue::try_from(v.to_string()).context(format!("invalid value: {v}"))?;

        self.info_key_values.insert(k, v);
        Ok(())
    }

    pub(super) fn build(
        &self,
        protocol: ThriftProtocol,
        seq_id: i32,
        buf: &mut Vec<u8>,
    ) -> anyhow::Result<HeaderBufOffsets> {
        let length_offset = buf.len();
        buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // LENGTH
        buf.extend_from_slice(&[0x0f, 0xff]); // HEADER MAGIC
        buf.extend_from_slice(&[0x00, 0x00]); // FLAGS
        let seq_id_bytes = seq_id.to_le_bytes();
        buf.extend_from_slice(&seq_id_bytes); // SEQUENCE NUMBER
        buf.extend_from_slice(&[0x00, 0x00]); // HEADER SIZE, bytes/4

        let content_offset = buf.len();

        // PROTOCOL ID (varint)
        let protocol_id = match protocol {
            ThriftProtocol::Binary => 0i32,
            ThriftProtocol::Compact => 2i32,
        };
        varint_encode(protocol_id, buf);

        // NUM TRANSFORMS (varint)
        varint_encode(0i32, buf);

        // INFO_KEYVALUE
        varint_encode(1i32, buf);
        let Ok(kv_count) = i32::try_from(self.info_key_values.len()) else {
            return Err(anyhow!("too many INFO_KEYVALUE headers"));
        };
        varint_encode(kv_count, buf);
        for (k, v) in self.info_key_values.iter() {
            buf.extend_from_slice(&k.len_bytes);
            buf.extend_from_slice(k.value.as_bytes());
            buf.extend_from_slice(&v.len_bytes);
            if !v.value.is_empty() {
                buf.extend_from_slice(v.value.as_bytes());
            }
        }

        // Update HEADER_SIZE field
        let header_size_bytes = buf.len() - content_offset;
        let mut header_size = header_size_bytes / 4;
        let left_bytes = header_size_bytes % 4;
        if left_bytes != 0 {
            buf.resize(buf.len() + 4 - left_bytes, 0);
            header_size += 1;
        }
        let b = header_size.to_be_bytes();
        buf[length_offset + 12] = b[0];
        buf[length_offset + 13] = b[1];

        Ok(HeaderBufOffsets {
            length: length_offset,
        })
    }

    pub(super) fn update_length(
        &self,
        offsets: HeaderBufOffsets,
        buf: &mut [u8],
    ) -> anyhow::Result<()> {
        let len = buf.len() - offsets.length - 4;
        let Ok(len) = u32::try_from(len) else {
            return Err(anyhow!("too value {len} for length"));
        };

        let len_bytes = len.to_le_bytes();
        let dst = &mut buf[offsets.length..];
        unsafe {
            std::ptr::copy_nonoverlapping(len_bytes.as_ptr(), dst.as_mut_ptr(), 4);
        }
        Ok(())
    }
}

fn varint_encode<T>(v: T, buf: &mut Vec<u8>)
where
    T: VarInt,
{
    let write_offset = buf.len();
    buf.resize(write_offset + v.required_space(), 0);
    v.encode_var(&mut buf[write_offset..]);
}
