/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeMap;
use std::convert::TryFrom;

use anyhow::{Context, anyhow};

use g3_types::codec::ThriftVarIntEncoder;

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
        let len = i16::try_from(value.len()).map_err(|_| {
            anyhow!(
                "too long Thrift THeader string value length {}",
                value.len()
            )
        })?;
        let mut encoder = ThriftVarIntEncoder::default();
        Ok(StringValue {
            len_bytes: encoder.encode_positive_i32(len as i32).to_vec(),
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

    pub(crate) fn build(
        &self,
        protocol: ThriftProtocol,
        seq_id: i32,
        buf: &mut Vec<u8>,
    ) -> anyhow::Result<HeaderBufOffsets> {
        let length_offset = buf.len();
        buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // LENGTH
        buf.extend_from_slice(&[0x0f, 0xff]); // HEADER MAGIC
        buf.extend_from_slice(&[0x00, 0x00]); // FLAGS
        let seq_id_bytes = seq_id.to_be_bytes();
        buf.extend_from_slice(&seq_id_bytes); // SEQUENCE NUMBER
        buf.extend_from_slice(&[0x00, 0x00]); // HEADER SIZE, bytes/4

        let content_offset = buf.len();

        let mut encoder = ThriftVarIntEncoder::default();

        // PROTOCOL ID (varint, i32)
        // See `THeaderProtocolID` in
        // https://github.com/apache/thrift/blob/master/lib/go/thrift/header_transport.go
        let protocol_id = match protocol {
            ThriftProtocol::Binary => 0i32,
            ThriftProtocol::Compact => 2i32,
        };
        buf.extend_from_slice(encoder.encode_positive_i32(protocol_id));

        // NUM TRANSFORMS (varint, i32)
        buf.extend_from_slice(encoder.encode_positive_i32(0));

        // INFO_KEYVALUE
        if !self.info_key_values.is_empty() {
            buf.extend_from_slice(encoder.encode_positive_i32(1));
            let kv_count = i32::try_from(self.info_key_values.len())
                .map_err(|_| anyhow!("too many INFO_KEYVALUE headers"))?;
            buf.extend_from_slice(encoder.encode_positive_i32(kv_count));
            for (k, v) in self.info_key_values.iter() {
                buf.extend_from_slice(&k.len_bytes);
                buf.extend_from_slice(k.value.as_bytes());
                buf.extend_from_slice(&v.len_bytes);
                if !v.value.is_empty() {
                    buf.extend_from_slice(v.value.as_bytes());
                }
            }
        }

        // Update HEADER_SIZE field
        let header_size_bytes = buf.len() - content_offset;
        let mut header_size = header_size_bytes / 4;
        let left_bytes = header_size_bytes % 4;
        if left_bytes != 0 {
            // padding to multiple of 4 bytes
            buf.resize(buf.len() + 4 - left_bytes, 0);
            header_size += 1;
        }
        let header_size = u16::try_from(header_size)
            .map_err(|_| anyhow!("too large Thrift THeader header size {header_size}"))?;
        let b = header_size.to_be_bytes();
        buf[length_offset + 12] = b[0];
        buf[length_offset + 13] = b[1];

        Ok(HeaderBufOffsets {
            length: length_offset,
            seq_id: length_offset + 8,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_simple() {
        let builder = ThriftTHeaderBuilder::default();
        let mut buf = Vec::with_capacity(0);
        let offsets = builder.build(ThriftProtocol::Binary, 0, &mut buf).unwrap();
        buf.extend_from_slice(&[
            0x80, 0x1, 0x0, 0x1, 0x0, 0x0, 0x0, 0x8, 0x74, 0x65, 0x73, 0x74, 0x56, 0x6f, 0x69,
            0x64, 0x0, 0x0, 0x0, 0x1, 0x0,
        ]);
        offsets.update_seq_id(&mut buf, 1).unwrap();
        offsets.update_length(&mut buf).unwrap();
        assert_eq!(
            &buf,
            &[
                0x0, 0x0, 0x0, 0x23, 0xf, 0xff, 0x0, 0x0, 0x0, 0x0, 0x0, 0x1, 0x0, 0x1, 0x0, 0x0,
                0x0, 0x0, 0x80, 0x1, 0x0, 0x1, 0x0, 0x0, 0x0, 0x8, 0x74, 0x65, 0x73, 0x74, 0x56,
                0x6f, 0x69, 0x64, 0x0, 0x0, 0x0, 0x1, 0x0,
            ]
        );
    }
}
