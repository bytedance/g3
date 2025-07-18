/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use integer_encoding::VarInt;

pub(crate) struct CompactRequestBuilder {
    name: String,
    name_len_bytes: Vec<u8>,
    payload: Vec<u8>,
}

impl CompactRequestBuilder {
    pub(crate) fn new_call(name: &str, payload: Vec<u8>) -> anyhow::Result<Self> {
        let Ok(name_len) = i32::try_from(name.len()) else {
            return Err(anyhow!("too long method name"));
        };
        let name_len_bytes = name_len.encode_var_vec();

        Ok(CompactRequestBuilder {
            name: name.to_string(),
            name_len_bytes,
            payload,
        })
    }

    pub(super) fn build(&self, seq_id: i32, framed: bool, buf: &mut Vec<u8>) -> anyhow::Result<()> {
        let start_offset = buf.len();
        if framed {
            buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        }

        // set fixed bits and message type to "Call"
        buf.extend_from_slice(&[0x82, 0x21]);

        let seq_id_size = seq_id.required_space();
        let seq_id_offset = buf.len();
        buf.resize(seq_id_offset + seq_id_size, 0);
        seq_id.encode_var(&mut buf[seq_id_offset..]);

        buf.extend_from_slice(&self.name_len_bytes);
        buf.extend_from_slice(self.name.as_bytes());

        buf.extend_from_slice(&self.payload);

        if framed {
            let len = buf.len() - start_offset;
            if len > 1638400 {
                return Err(anyhow!("too large length {len} for framed transport"));
            }

            let bytes = (len as i32).to_be_bytes();
            let dst = &mut buf[start_offset..];
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst.as_mut_ptr(), 4);
            }
        }

        Ok(())
    }
}
