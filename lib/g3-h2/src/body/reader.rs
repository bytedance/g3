/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use bytes::{Buf, Bytes};
use h2::{FlowControl, RecvStream};
use tokio::io::{AsyncRead, ReadBuf};

pub struct H2StreamReader {
    recv_stream: RecvStream,
    recv_flow_control: FlowControl,
    received_bytes: Option<Bytes>,
}

impl H2StreamReader {
    pub fn new(mut stream: RecvStream) -> Self {
        let recv_flow_control = stream.flow_control().clone();
        H2StreamReader {
            recv_stream: stream,
            recv_flow_control,
            received_bytes: None,
        }
    }
}

impl AsyncRead for H2StreamReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        loop {
            if let Some(mut b) = self.received_bytes.take() {
                let to_put = buf.remaining().min(b.len());
                return match self.recv_flow_control.release_capacity(to_put) {
                    Ok(_) => {
                        let split = b.split_to(to_put);
                        buf.put_slice(&split);
                        if b.has_remaining() {
                            self.received_bytes = Some(b);
                        }
                        Poll::Ready(Ok(()))
                    }
                    Err(e) => {
                        self.received_bytes = Some(b);
                        if e.is_io() {
                            Poll::Ready(Err(e.into_io().unwrap()))
                        } else {
                            Poll::Ready(Err(io::Error::other(e)))
                        }
                    }
                };
            } else {
                match ready!(self.recv_stream.poll_data(cx)) {
                    Some(Ok(b)) => {
                        if b.is_empty() {
                            continue;
                        }
                        self.received_bytes = Some(b);
                    }
                    Some(Err(e)) => {
                        return if e.is_io() {
                            Poll::Ready(Err(e.into_io().unwrap()))
                        } else {
                            Poll::Ready(Err(io::Error::other(e)))
                        };
                    }
                    None => return Poll::Ready(Ok(())),
                };
            }
        }
    }
}
