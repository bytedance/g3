/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use bytes::Bytes;
use h2::SendStream;
use tokio::io::AsyncWrite;

pub struct H2StreamWriter {
    send_stream: SendStream<Bytes>,
}

impl H2StreamWriter {
    pub fn new(stream: SendStream<Bytes>) -> Self {
        H2StreamWriter {
            send_stream: stream,
        }
    }
}

impl AsyncWrite for H2StreamWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.send_stream.reserve_capacity(buf.len());
        match ready!(self.send_stream.poll_capacity(cx)) {
            Some(Ok(0)) => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "unexpected write of zero bytes",
            ))),
            Some(Ok(n)) => {
                match self
                    .send_stream
                    .send_data(Bytes::copy_from_slice(&buf[0..n]), false)
                {
                    Ok(_) => Poll::Ready(Ok(n)),
                    Err(e) => {
                        if e.is_io() {
                            Poll::Ready(Err(e.into_io().unwrap()))
                        } else {
                            Poll::Ready(Err(io::Error::other(e)))
                        }
                    }
                }
            }
            Some(Err(e)) => {
                if e.is_io() {
                    Poll::Ready(Err(e.into_io().unwrap()))
                } else {
                    Poll::Ready(Err(io::Error::other(e)))
                }
            }
            None => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "unexpected write of zero bytes",
            ))),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Err(e) = self.send_stream.send_data(Bytes::new(), true) {
            if e.is_io() {
                Poll::Ready(Err(e.into_io().unwrap()))
            } else {
                Poll::Ready(Err(io::Error::other(e)))
            }
        } else {
            Poll::Ready(Ok(()))
        }
    }
}
