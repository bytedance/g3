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

use std::io::{self, Read, Write};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{ready, Context, Poll, Waker};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use g3_types::net::ClientHelloRewriteRule;

pub(crate) struct SslIoWrapper<S> {
    io: S,
    waker: Option<Waker>,
    client_hello_rewriter: Option<ClientHelloRewriter>,
}

impl<S> SslIoWrapper<S> {
    pub(crate) fn new(io: S) -> Self {
        SslIoWrapper {
            io,
            waker: None,
            client_hello_rewriter: None,
        }
    }

    pub(crate) fn set_client_hello_rewriter(&mut self, rule: Option<Arc<ClientHelloRewriteRule>>) {
        self.client_hello_rewriter = rule.map(ClientHelloRewriter::new);
    }

    #[inline]
    pub(crate) fn set_cx(&mut self, cx: &mut Context<'_>) {
        self.waker = Some(cx.waker().clone());
    }

    #[inline]
    pub(crate) fn get_mut(&mut self) -> &mut S {
        &mut self.io
    }

    #[inline]
    pub(crate) fn get_pin_mut(&mut self) -> Pin<&mut S>
    where
        S: Unpin,
    {
        Pin::new(&mut self.io)
    }

    fn with_context<F, R>(&mut self, mut f: F) -> R
    where
        F: FnMut(Pin<&mut S>, &mut Context<'_>, Option<&mut ClientHelloRewriter>) -> R,
        S: Unpin,
    {
        let stream = Pin::new(&mut self.io);
        let mut context =
            Context::from_waker(self.waker.as_ref().expect("async context waker is not set"));
        f(stream, &mut context, self.client_hello_rewriter.as_mut())
    }
}

impl<S: AsyncRead + Unpin> Read for SslIoWrapper<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.with_context(|stream, cx, _| {
            let mut buf = ReadBuf::new(buf);
            match stream.poll_read(cx, &mut buf) {
                Poll::Ready(Ok(_)) => Ok(buf.filled().len()),
                Poll::Ready(Err(e)) => Err(e),
                Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
            }
        })
    }
}

impl<S: AsyncWrite + Unpin> Write for SslIoWrapper<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.with_context(|stream, cx, client_hello_rewriter| {
            let r = if let Some(rewriter) = client_hello_rewriter {
                rewriter.poll_write(buf, stream, cx)
            } else {
                stream.poll_write(cx, buf)
            };
            match r {
                Poll::Ready(r) => r,
                Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
            }
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.with_context(|stream, cx, client_hello_rewriter| {
            let r = if let Some(rewriter) = client_hello_rewriter {
                rewriter.poll_flush(stream, cx)
            } else {
                stream.poll_flush(cx)
            };
            match r {
                Poll::Ready(r) => r,
                Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
            }
        })
    }
}

enum ClientHelloRewriteState {
    ReadHead(usize),
    ReadData(usize),
    WriteBuf(usize),
    WriteDone,
    Finish,
}

struct ClientHelloRewriter {
    state: ClientHelloRewriteState,
    buf: Vec<u8>,
    rule: Arc<ClientHelloRewriteRule>,
}

impl ClientHelloRewriter {
    fn new(rule: Arc<ClientHelloRewriteRule>) -> Self {
        ClientHelloRewriter {
            state: ClientHelloRewriteState::ReadHead(5),
            buf: Vec::with_capacity(512),
            rule,
        }
    }
}

impl ClientHelloRewriter {
    fn poll_write<S>(
        &mut self,
        data: &[u8],
        stream: Pin<&mut S>,
        cx: &mut Context,
    ) -> Poll<io::Result<usize>>
    where
        S: AsyncWrite,
    {
        let mut offset = 0;
        loop {
            let left = data.len() - offset;
            match self.state {
                ClientHelloRewriteState::ReadHead(n) => {
                    if left > n {
                        self.buf.extend_from_slice(&data[offset..offset + n]);
                        offset += n;
                        debug_assert_eq!(self.buf[0], 0x16);
                        let data_len = u16::from_be_bytes([self.buf[3], self.buf[4]]);
                        self.state = ClientHelloRewriteState::ReadData(data_len as usize);
                    } else {
                        self.buf.extend_from_slice(&data[offset..]);
                        self.state = ClientHelloRewriteState::ReadHead(n - left);
                        return Poll::Ready(Ok(offset + left));
                    }
                }
                ClientHelloRewriteState::ReadData(n) => {
                    if left < n {
                        self.buf.extend_from_slice(&data[offset..]);
                        self.state = ClientHelloRewriteState::ReadData(n - left);
                        return Poll::Ready(Ok(offset + left));
                    } else {
                        self.buf.extend_from_slice(&data[offset..offset + left]);
                        offset += left;
                        self.rewrite_buf()?;
                        self.state = ClientHelloRewriteState::WriteBuf(0);
                    }
                }
                ClientHelloRewriteState::WriteBuf(mut b_offset) => {
                    return match stream.poll_write(cx, &self.buf[b_offset..]) {
                        Poll::Pending => Poll::Ready(Ok(offset)),
                        Poll::Ready(Ok(nw)) => {
                            b_offset += nw;
                            if b_offset >= self.buf.len() {
                                self.buf.clear();
                                self.state = ClientHelloRewriteState::WriteDone;
                            } else {
                                self.state = ClientHelloRewriteState::WriteBuf(b_offset);
                            }
                            Poll::Ready(Ok(offset))
                        }
                        Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                    }
                }
                ClientHelloRewriteState::WriteDone => {
                    // TODO track failed session resumption and restart rewrite
                    self.state = ClientHelloRewriteState::Finish
                }
                ClientHelloRewriteState::Finish => return stream.poll_write(cx, data),
            }
        }
    }

    fn poll_flush<S>(&mut self, mut stream: Pin<&mut S>, cx: &mut Context) -> Poll<io::Result<()>>
    where
        S: AsyncWrite,
    {
        loop {
            match self.state {
                ClientHelloRewriteState::ReadHead(_) | ClientHelloRewriteState::ReadData(_) => {
                    return Poll::Ready(Ok(()));
                }
                ClientHelloRewriteState::WriteBuf(mut b_offset) => {
                    let nw = ready!(stream.as_mut().poll_write(cx, &self.buf[b_offset..]))?;
                    b_offset += nw;
                    if b_offset >= self.buf.len() {
                        self.buf.clear();
                        self.state = ClientHelloRewriteState::Finish;
                    } else {
                        self.state = ClientHelloRewriteState::WriteBuf(b_offset);
                    }
                }
                ClientHelloRewriteState::WriteDone | ClientHelloRewriteState::Finish => {
                    return stream.poll_flush(cx)
                }
            }
        }
    }

    fn rewrite_buf(&mut self) -> io::Result<()> {
        let output = self
            .rule
            .rewrite(&self.buf)
            .ok_or(io::Error::other("failed to rewrite client hello message"))?;
        self.buf = output;
        Ok(())
    }
}
