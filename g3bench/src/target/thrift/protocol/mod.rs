/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

mod binary;
pub(super) use binary::BinaryRequestBuilder;

mod compact;
pub(super) use compact::CompactRequestBuilder;

pub(super) enum ThriftRequestBuilder {
    Binary(BinaryRequestBuilder),
    Compact(CompactRequestBuilder),
}

impl ThriftRequestBuilder {
    pub(super) fn build(&self, seq_id: i32, buf: &mut Vec<u8>) -> anyhow::Result<()> {
        match self {
            ThriftRequestBuilder::Binary(r) => r.build(seq_id, buf),
            ThriftRequestBuilder::Compact(r) => r.build(seq_id, buf),
        }
    }
}
