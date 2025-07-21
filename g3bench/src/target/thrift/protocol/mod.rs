/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

mod binary;
pub(super) use binary::BinaryMessageBuilder;

mod compact;
pub(super) use compact::CompactMessageBuilder;

pub(super) enum ThriftProtocol {
    Binary,
    Compact,
}

pub(super) enum ThriftMessageBuilder {
    Binary(BinaryMessageBuilder),
    Compact(CompactMessageBuilder),
}

impl ThriftMessageBuilder {
    pub(super) fn build_call(
        &self,
        seq_id: i32,
        framed: bool,
        payload: &[u8],
        buf: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        match self {
            ThriftMessageBuilder::Binary(r) => r.build_call(seq_id, framed, payload, buf),
            ThriftMessageBuilder::Compact(r) => r.build_call(seq_id, framed, payload, buf),
        }
    }

    pub(super) fn protocol(&self) -> ThriftProtocol {
        match self {
            ThriftMessageBuilder::Binary(_) => ThriftProtocol::Binary,
            ThriftMessageBuilder::Compact(_) => ThriftProtocol::Compact,
        }
    }
}
