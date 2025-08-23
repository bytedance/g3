/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use bytes::Bytes;
use h2::RecvStream;
use tokio::io::{AsyncRead, ReadBuf};

pub struct H2StreamReader {
    recv_stream: RecvStream,
    received_bytes: Option<Bytes>,
}

impl H2StreamReader {
    pub fn new(stream: RecvStream) -> Self {
        H2StreamReader {
            recv_stream: stream,
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
                let split = b.split_to(to_put);
                buf.put_slice(&split);
                if !b.is_empty() {
                    self.received_bytes = Some(b);
                }
                return Poll::Ready(Ok(()));
            }

            match ready!(self.recv_stream.poll_data(cx)) {
                Some(Ok(b)) => {
                    if b.is_empty() {
                        continue;
                    }
                    self.recv_stream
                        .flow_control()
                        .release_capacity(b.len())
                        .map_err(|e| {
                            if e.is_io() {
                                e.into_io().unwrap()
                            } else {
                                io::Error::other(e)
                            }
                        })?;
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
