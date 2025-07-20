/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::FutureExt;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

use g3_io_ext::LimitedWriteExt;

use super::{ThriftTcpResponse, ThriftTcpResponseError, ThriftTcpResponseLocalError};
use crate::target::thrift::tcp::ThriftTcpArgs;

pub(crate) struct SimplexTransfer {
    args: Arc<ThriftTcpArgs>,
    reader: Box<dyn AsyncRead + Send + Sync + Unpin>,
    writer: Box<dyn AsyncWrite + Send + Sync + Unpin>,
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
    ) -> Self
    where
        R: AsyncRead + Send + Sync + Unpin + 'static,
        W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        SimplexTransfer {
            args,
            reader: Box::new(reader),
            writer: Box::new(writer),
            next_req_id: 0,
            read_buf: Vec::with_capacity(1024),
            local_addr,
        }
    }

    pub(crate) fn is_closed(&mut self) -> bool {
        let mut buf = [0u8; 4];
        self.reader.read(&mut buf).now_or_never().is_some()
    }

    #[inline]
    pub(crate) fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub(crate) async fn send_request(
        &mut self,
        req_payload: &[u8],
    ) -> Result<ThriftTcpResponse, ThriftTcpResponseError> {
        let req = self
            .args
            .build_tcp_request(self.next_req_id, req_payload)
            .map_err(ThriftTcpResponseLocalError::InvalidRequest)?;
        self.next_req_id = self.next_req_id.wrapping_add(1);

        self.writer
            .write_all_flush(&req.buf)
            .await
            .map_err(ThriftTcpResponseLocalError::WriteFailed)?;

        self.args
            .read_tcp_response(&mut self.reader, &mut self.read_buf)
            .await
    }
}
