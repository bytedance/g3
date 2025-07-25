/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use tokio::io::AsyncRead;

use super::ThriftTcpResponseError;
use crate::target::thrift::protocol::ThriftProtocol;

mod kitex;
pub(super) use kitex::{KitexTTHeaderBuilder, KitexTTHeaderReader};

mod thrift;
pub(super) use thrift::{ThriftTHeaderBuilder, ThriftTHeaderReader};

pub(super) struct HeaderBufOffsets {
    length: usize,
    seq_id: usize,
}

impl HeaderBufOffsets {
    pub(super) fn update_seq_id(&self, buf: &mut [u8], seq_id: i32) -> anyhow::Result<()> {
        let seq_id_bytes = seq_id.to_be_bytes();
        let dst = &mut buf[self.seq_id..];
        unsafe {
            std::ptr::copy_nonoverlapping(seq_id_bytes.as_ptr(), dst.as_mut_ptr(), 4);
        }
        Ok(())
    }

    pub(super) fn update_length(&self, buf: &mut [u8]) -> anyhow::Result<()> {
        let len = buf.len() - self.length - 4;
        let len = u32::try_from(len).map_err(|_| anyhow!("too large length {len}"))?;

        let len_bytes = len.to_be_bytes();
        let dst = &mut buf[self.length..];
        unsafe {
            std::ptr::copy_nonoverlapping(len_bytes.as_ptr(), dst.as_mut_ptr(), 4);
        }
        Ok(())
    }
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

    pub(super) fn response_reader(&self) -> HeaderReader {
        match self {
            HeaderBuilder::Thrift(_) => HeaderReader::Thrift(Default::default()),
            HeaderBuilder::Kitex(_) => HeaderReader::Kitex(Default::default()),
        }
    }
}

pub(super) struct HeaderTransportResponse<'a> {
    pub(super) seq_id: i32,
    pub(super) frame_bytes: &'a [u8],
}

pub(super) enum HeaderReader {
    Thrift(ThriftTHeaderReader),
    Kitex(KitexTTHeaderReader),
}

impl HeaderReader {
    pub(super) async fn read<'a, R>(
        &mut self,
        reader: &mut R,
        buf: &'a mut Vec<u8>,
    ) -> Result<HeaderTransportResponse<'a>, ThriftTcpResponseError>
    where
        R: AsyncRead + Unpin,
    {
        match self {
            HeaderReader::Thrift(t) => t.read(reader, buf).await,
            HeaderReader::Kitex(t) => t.read(reader, buf).await,
        }
    }
}
