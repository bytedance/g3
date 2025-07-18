/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use crate::target::thrift::protocol::ThriftProtocol;

mod kitex;
pub(super) use kitex::KitexTTHeaderBuilder;

mod thrift;
pub(super) use thrift::ThriftTHeaderBuilder;

pub(super) struct HeaderBufOffsets {
    length: usize,
}

pub(super) enum HeaderBuilder {
    Thrift(ThriftTHeaderBuilder),
    Kitex(KitexTTHeaderBuilder),
}

impl HeaderBuilder {
    pub(super) fn build(
        &self,
        protocol: ThriftProtocol,
        seq_id: i32,
        buf: &mut Vec<u8>,
    ) -> anyhow::Result<HeaderBufOffsets> {
        match self {
            HeaderBuilder::Thrift(t) => t.build(protocol, seq_id, buf),
            HeaderBuilder::Kitex(t) => t.build(protocol, seq_id, buf),
        }
    }

    pub(super) fn update_length(
        &self,
        offsets: HeaderBufOffsets,
        buf: &mut [u8],
    ) -> anyhow::Result<()> {
        match self {
            HeaderBuilder::Thrift(t) => t.update_length(offsets, buf),
            HeaderBuilder::Kitex(t) => t.update_length(offsets, buf),
        }
    }
}
