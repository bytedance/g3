/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;

pub(crate) struct BinaryMessageBuilder {
    name: String,
    name_len_bytes: [u8; 4],
}

impl BinaryMessageBuilder {
    pub(crate) fn new_call(name: &str) -> anyhow::Result<Self> {
        let name_len = i32::try_from(name.len()).map_err(|_| anyhow!("too long method name"))?;
        let name_len_bytes = name_len.to_be_bytes();

        Ok(BinaryMessageBuilder {
            name: name.to_string(),
            name_len_bytes,
        })
    }

    pub(crate) fn build_call(
        &self,
        seq_id: i32,
        framed: bool,
        payload: &[u8],
        buf: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        if framed {
            let len = 4 + self.name_len_bytes.len() + self.name.len() + 4 + payload.len();
            let len = u32::try_from(len).map_err(|_| anyhow!("too large frame size {len}"))?;
            buf.extend_from_slice(&len.to_be_bytes());
        }

        // set fixed bits and message type to "Call"
        buf.extend_from_slice(&[0x80, 0x01, 0x00, 0x01]);

        buf.extend_from_slice(&self.name_len_bytes);
        buf.extend_from_slice(self.name.as_bytes());

        buf.extend_from_slice(&seq_id.to_be_bytes());

        buf.extend_from_slice(payload);

        Ok(())
    }
}
