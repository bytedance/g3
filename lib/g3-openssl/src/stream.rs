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
use std::task::{Context, Poll};

use openssl::ssl::{self, SslRef};
use openssl_sys::{SSL_ERROR_WANT_READ, SSL_ERROR_WANT_WRITE, SSL_ERROR_ZERO_RETURN};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use super::error::{SSL_ERROR_WANT_ASYNC, SSL_ERROR_WANT_ASYNC_JOB};
use super::SslIoWrapper;

pub struct SslStream<S> {
    inner: ssl::SslStream<SslIoWrapper<S>>,
}

impl<S> SslStream<S> {
    #[inline]
    pub(crate) fn new(inner: ssl::SslStream<SslIoWrapper<S>>) -> Self {
        SslStream { inner }
    }

    #[inline]
    pub fn ssl(&self) -> &SslRef {
        self.inner.ssl()
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut S {
        self.inner.get_mut().get_mut()
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncRead for SslStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.inner.get_mut().set_cx(cx);

        loop {
            match self.inner.ssl_read_uninit(unsafe { buf.unfilled_mut() }) {
                Ok(n) => {
                    unsafe { buf.assume_init(n) };
                    buf.advance(n);
                    return Poll::Ready(Ok(()));
                }
                Err(e) => match e.code().as_raw() {
                    SSL_ERROR_ZERO_RETURN => return Poll::Ready(Ok(())),
                    SSL_ERROR_WANT_READ => {
                        if e.io_error().is_none() {
                            continue;
                        } else {
                            return Poll::Pending;
                        }
                    }
                    SSL_ERROR_WANT_WRITE => return Poll::Pending,
                    SSL_ERROR_WANT_ASYNC => {
                        // TODO
                        todo!()
                    }
                    SSL_ERROR_WANT_ASYNC_JOB => {
                        cx.waker().wake_by_ref();
                        return Poll::Pending;
                    }
                    _ => {
                        return Poll::Ready(Err(e.into_io_error().unwrap_or_else(io::Error::other)))
                    }
                },
            }
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncWrite for SslStream<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.inner.get_mut().set_cx(cx);

        loop {
            match self.inner.ssl_write(buf) {
                Ok(n) => return Poll::Ready(Ok(n)),
                Err(e) => match e.code().as_raw() {
                    SSL_ERROR_WANT_READ => {
                        if e.io_error().is_none() {
                            continue;
                        } else {
                            return Poll::Pending;
                        }
                    }
                    SSL_ERROR_WANT_WRITE => return Poll::Pending,
                    SSL_ERROR_WANT_ASYNC => {
                        // TODO
                        todo!()
                    }
                    SSL_ERROR_WANT_ASYNC_JOB => {
                        cx.waker().wake_by_ref();
                        return Poll::Pending;
                    }
                    _ => {
                        return Poll::Ready(Err(e.into_io_error().unwrap_or_else(io::Error::other)))
                    }
                },
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.inner.get_mut().get_pin_mut().poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.inner.get_mut().set_cx(cx);

        if let Err(e) = self.inner.shutdown() {
            match e.code().as_raw() {
                SSL_ERROR_ZERO_RETURN => {}
                SSL_ERROR_WANT_READ | SSL_ERROR_WANT_WRITE => return Poll::Pending,
                SSL_ERROR_WANT_ASYNC => {
                    // TODO
                    todo!()
                }
                SSL_ERROR_WANT_ASYNC_JOB => {
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                _ => {
                    return Poll::Ready(Err(e.into_io_error().unwrap_or_else(io::Error::other)));
                }
            }
        }

        self.inner.get_mut().get_pin_mut().poll_shutdown(cx)
    }
}
