/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use integer_encoding::VarInt;

pub(crate) struct CompactMessageBuilder {
    name: String,
    name_len_bytes: Vec<u8>,
}

impl CompactMessageBuilder {
    pub(crate) fn new_call(name: &str) -> anyhow::Result<Self> {
        // the name length is encoded from an unsigned integer, which is different from
        // https://github.com/apache/thrift/blob/master/doc/specs/thrift-compact-protocol.md
        let name_len = u32::try_from(name.len()).map_err(|_| anyhow!("too long method name"))?;
        let name_len_bytes = name_len.encode_var_vec();

        Ok(CompactMessageBuilder {
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

        buf.extend_from_slice(payload);

        if framed {
            let len = buf.len() - start_offset - 4;
            let len = i32::try_from(len).map_err(|_| anyhow!("too large frame size {len}"))?;
            let bytes = len.to_be_bytes();
            let dst = &mut buf[start_offset..];
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst.as_mut_ptr(), 4);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_call() {
        let builder = CompactMessageBuilder::new_call("ping").unwrap();
        let mut buf = Vec::new();
        builder.build_call(-1, true, &[0x00], &mut buf).unwrap();
        assert_eq!(
            &buf,
            &[
                0x0, 0x0, 0x0, 0x9, 0x82, 0x21, 0x1, 0x4, 0x70, 0x69, 0x6e, 0x67, 0x0
            ]
        );
    }
}
