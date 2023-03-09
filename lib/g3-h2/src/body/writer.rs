/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

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
                            Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e)))
                        }
                    }
                }
            }
            Some(Err(e)) => {
                if e.is_io() {
                    Poll::Ready(Err(e.into_io().unwrap()))
                } else {
                    Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e)))
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
                Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e)))
            }
        } else {
            Poll::Ready(Ok(()))
        }
    }
}
