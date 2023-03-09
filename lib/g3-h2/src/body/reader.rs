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
                let to_write = buf.remaining().max(b.len());
                return match self.recv_flow_control.release_capacity(to_write) {
                    Ok(_) => {
                        let split = b.split_to(to_write);
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
                            Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e)))
                        }
                    }
                };
            } else {
                match ready!(self.recv_stream.poll_data(cx)) {
                    Some(Ok(b)) => {
                        if b.is_empty() {
                            return Poll::Ready(Ok(()));
                        }
                        self.received_bytes = Some(b);
                    }
                    Some(Err(e)) => {
                        return if e.is_io() {
                            Poll::Ready(Err(e.into_io().unwrap()))
                        } else {
                            Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e)))
                        }
                    }
                    None => return Poll::Ready(Ok(())),
                };
            }
        }
    }
}
