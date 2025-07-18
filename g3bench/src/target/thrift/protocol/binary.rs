/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;

pub(crate) struct BinaryRequestBuilder {
    name: String,
    name_len_bytes: [u8; 4],
    framed: bool,
    payload: Vec<u8>,
}

impl BinaryRequestBuilder {
    pub(crate) fn new_call(name: &str, payload: Vec<u8>, framed: bool) -> anyhow::Result<Self> {
        if name.len() > i32::MAX as usize {
            return Err(anyhow!("too long method name"));
        }
        let name_len = name.len() as i32;
        let name_len_bytes = name_len.to_be_bytes();

        let len = 4 + 4 + name.len() + 4 + payload.len();
        if framed {
            if len > 1638400 {
                return Err(anyhow!("too large length {len} for framed transport"));
            }
        }

        Ok(BinaryRequestBuilder {
            name: name.to_string(),
            name_len_bytes,
            framed,
            payload,
        })
    }

    pub(super) fn build(&self, seq_id: i32, buf: &mut Vec<u8>) -> anyhow::Result<()> {
        let len = 4 + self.name_len_bytes.len() + self.name.len() + 4 + self.payload.len();
        if self.framed {
            if len > 1638400 {
                return Err(anyhow!("too large length {len} for framed transport"));
            }
            buf.extend_from_slice(&len.to_be_bytes());
        }

        // set fixed bits and message type to "Call"
        buf.extend_from_slice(&[0x80, 0x01, 0x00, 0x01]);

        buf.extend_from_slice(&self.name_len_bytes);
        buf.extend_from_slice(self.name.as_bytes());

        buf.extend_from_slice(&seq_id.to_be_bytes());

        buf.extend_from_slice(&self.payload);

        Ok(())
    }
}
