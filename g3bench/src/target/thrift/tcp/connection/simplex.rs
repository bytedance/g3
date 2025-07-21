/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::FutureExt;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

use g3_io_ext::LimitedWriteExt;

use super::{ThriftTcpResponse, ThriftTcpResponseError};
use crate::target::thrift::tcp::ThriftTcpArgs;
use crate::target::thrift::tcp::header::HeaderBufOffsets;

pub(crate) struct SimplexTransfer {
    args: Arc<ThriftTcpArgs>,
    reader: Box<dyn AsyncRead + Send + Sync + Unpin>,
    writer: Box<dyn AsyncWrite + Send + Sync + Unpin>,

    send_buf: Vec<u8>,
    send_header_size: usize,
    send_header_buf_offsets: Option<HeaderBufOffsets>,

    next_req_id: i32,

    read_buf: Vec<u8>,
    local_addr: SocketAddr,
}

impl SimplexTransfer {
    pub(crate) fn new<R, W>(
        args: Arc<ThriftTcpArgs>,
        reader: R,
        writer: W,
        local_addr: SocketAddr,
    ) -> anyhow::Result<Self>
    where
        R: AsyncRead + Send + Sync + Unpin + 'static,
        W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let mut send_buf = Vec::with_capacity(1024);
        let mut send_header_buf_offsets = None;
        if let Some(header_builder) = &args.header_builder {
            let offsets =
                header_builder.build(args.global.request_builder.protocol(), 0, &mut send_buf)?;
            send_header_buf_offsets = Some(offsets);
        }
        let send_header_size = send_buf.len();

        Ok(SimplexTransfer {
            args,
            reader: Box::new(reader),
            writer: Box::new(writer),
            send_buf,
            send_header_size,
            send_header_buf_offsets,
            next_req_id: 0,
            read_buf: Vec::with_capacity(1024),
            local_addr,
        })
    }

    pub(crate) fn is_closed(&mut self) -> bool {
        let mut buf = [0u8; 4];
        self.reader.read(&mut buf).now_or_never().is_some()
    }

    #[inline]
    pub(crate) fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    fn build_new_request(&mut self, req_payload: &[u8]) -> anyhow::Result<()> {
        let seq_id = self.next_req_id.max(1);
        if let Some(offsets) = &self.send_header_buf_offsets {
            offsets.update_seq_id(&mut self.send_buf, seq_id)?;

            self.send_buf.resize(self.send_header_size, 0);
            self.args.global.request_builder.build_call(
                seq_id,
                self.args.framed,
                req_payload,
                &mut self.send_buf,
            )?;

            offsets.update_length(&mut self.send_buf)?;
        } else {
            self.send_buf.clear();
            self.args.global.request_builder.build_call(
                seq_id,
                self.args.framed,
                req_payload,
                &mut self.send_buf,
            )?;
        }

        self.next_req_id = seq_id.wrapping_add(1);
        Ok(())
    }

    pub(crate) async fn send_request(
        &mut self,
        req_payload: &[u8],
    ) -> Result<ThriftTcpResponse, ThriftTcpResponseError> {
        self.build_new_request(req_payload)
            .map_err(ThriftTcpResponseError::InvalidRequest)?;

        self.writer
            .write_all_flush(&self.send_buf)
            .await
            .map_err(ThriftTcpResponseError::WriteFailed)?;

        self.args
            .read_tcp_response(&mut self.reader, &mut self.read_buf)
            .await
    }
}
