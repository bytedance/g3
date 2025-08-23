/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeMap;

use anyhow::{Context, anyhow};

use super::HeaderBufOffsets;
use crate::target::thrift::protocol::ThriftProtocol;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct UInt16Value {
    bytes: [u8; 2],
    value: u16,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct StringValue {
    len_bytes: [u8; 2],
    value: String,
}

impl TryFrom<String> for StringValue {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let len = u16::try_from(value.len()).map_err(|_| {
            anyhow!(
                "too long Kitex TTHeader string value length {}",
                value.len()
            )
        })?;
        Ok(StringValue {
            len_bytes: len.to_be_bytes(),
            value,
        })
    }
}

pub(crate) struct KitexTTHeaderBuilder {
    info_key_values: BTreeMap<StringValue, StringValue>,
    // See https://github.com/cloudwego/kitex/blob/main/pkg/remote/transmeta/metakey.go#L23
    // for all defined int key types
    info_int_key_values: BTreeMap<UInt16Value, StringValue>,
    acl_token: Option<StringValue>,
}

impl KitexTTHeaderBuilder {
    pub(crate) fn new_request(framed: bool, method: &str) -> anyhow::Result<Self> {
        let mut builder = KitexTTHeaderBuilder {
            info_key_values: Default::default(),
            info_int_key_values: Default::default(),
            acl_token: None,
        };

        // TRANSPORT_TYPE = 1
        if framed {
            builder.add_info_int_kv(1, "framed")?;
        } else {
            builder.add_info_int_kv(1, "unframed")?;
        }

        // LOG_ID = 2
        builder.add_info_int_kv(2, "")?;

        // FROM_SERVICE = 3
        builder.add_info_int_kv(3, "-")?;

        // FROM_CLUSTER = 4
        builder.add_info_int_kv(4, "default")?;

        // FROM_IDC = 5
        builder.add_info_int_kv(5, "")?;

        // TO_SERVICE = 6
        builder.add_info_int_kv(6, "")?;

        // TO_CLUSTER = 7
        builder.add_info_int_kv(7, "default")?;

        // TO_IDC = 8
        builder.add_info_int_kv(8, "")?;

        // TO_METHOD = 9
        builder.add_info_int_kv(9, method)?;

        // WithMeshHeader = 0x10, always set to "3", see
        // https://github.com/cloudwego/volo/blob/main/volo-thrift/src/codec/default/ttheader.rs#L227
        builder.add_info_int_kv(0x10, "3")?;

        Ok(builder)
    }

    pub(crate) fn add_info_kv(&mut self, k: &str, v: &str) -> anyhow::Result<()> {
        if k.is_empty() {
            return Err(anyhow!("empty key"));
        }
        let k = StringValue::try_from(k.to_string()).context(format!("invalid key: {k}"))?;
        let v = StringValue::try_from(v.to_string()).context(format!("invalid value: {v}"))?;

        self.info_key_values.insert(k, v);
        Ok(())
    }

    pub(crate) fn add_info_int_kv(&mut self, k: u16, v: &str) -> anyhow::Result<()> {
        let k = UInt16Value {
            bytes: k.to_be_bytes(),
            value: k,
        };
        let v = StringValue::try_from(v.to_string()).context(format!("invalid value: {v}"))?;

        self.info_int_key_values.insert(k, v);
        Ok(())
    }

    pub(crate) fn set_acl_token(&mut self, token: &str) -> anyhow::Result<()> {
        if token.is_empty() {
            return Err(anyhow!("empty token"));
        }
        let v = StringValue::try_from(token.to_string())?;
        self.acl_token = Some(v);
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
        buf.extend_from_slice(&[0x10, 0x00]); // HEADER MAGIC
        buf.extend_from_slice(&[0x00, 0x00]); // FLAGS
        let seq_id_bytes = seq_id.to_be_bytes();
        buf.extend_from_slice(&seq_id_bytes); // SEQUENCE NUMBER
        buf.extend_from_slice(&[0x00, 0x00]); // HEADER SIZE, bytes/4

        let content_offset = buf.len();

        // PROTOCOL ID u8
        match protocol {
            ThriftProtocol::Binary => buf.push(0x00),
            ThriftProtocol::Compact => buf.push(0x02),
        }

        // NUM TRANSFORMS u8
        buf.push(0x00);

        // INFO_KEYVALUE
        buf.push(0x01);
        let kv_count = u16::try_from(self.info_key_values.len())
            .map_err(|_| anyhow!("too many INFO_KEYVALUE headers"))?;
        let b = kv_count.to_be_bytes();
        buf.push(b[0]);
        buf.push(b[1]);
        for (k, v) in self.info_key_values.iter() {
            buf.extend_from_slice(&k.len_bytes);
            buf.extend_from_slice(k.value.as_bytes());
            buf.extend_from_slice(&v.len_bytes);
            if !v.value.is_empty() {
                buf.extend_from_slice(v.value.as_bytes());
            }
        }

        // INFO_INTKEYVALUE
        buf.push(0x10);
        let kv_count = u16::try_from(self.info_int_key_values.len())
            .map_err(|_| anyhow!("too many INFO_INTKEYVALUE headers"))?;
        let b = kv_count.to_be_bytes();
        buf.push(b[0]);
        buf.push(b[1]);
        for (k, v) in self.info_int_key_values.iter() {
            buf.extend_from_slice(&k.bytes);
            buf.extend_from_slice(&v.len_bytes);
            if !v.value.is_empty() {
                buf.extend_from_slice(v.value.as_bytes());
            }
        }

        // ACL_TOKEN_KEYVALUE
        if let Some(v) = &self.acl_token {
            buf.push(0x11);
            buf.extend_from_slice(&v.len_bytes);
            buf.extend_from_slice(v.value.as_bytes());
        }

        // Update HEADER_SIZE field
        let header_size_bytes = buf.len() - content_offset;
        let mut header_size = header_size_bytes / 4;
        let left_bytes = header_size_bytes % 4;
        if left_bytes != 0 {
            buf.resize(buf.len() + 4 - left_bytes, 0);
            header_size += 1;
        }
        let header_size = u16::try_from(header_size)
            .map_err(|_| anyhow!("too large Kitex TTHeader header size {header_size}"))?;
        let b = header_size.to_be_bytes();
        buf[length_offset + 12] = b[0];
        buf[length_offset + 13] = b[1];

        Ok(HeaderBufOffsets {
            length: length_offset,
            seq_id: length_offset + 8,
        })
    }
}
