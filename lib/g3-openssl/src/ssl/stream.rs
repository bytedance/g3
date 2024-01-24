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
#[cfg(feature = "async-job")]
use std::task::ready;
use std::task::{Context, Poll};

use openssl::ssl::{self, ErrorCode, SslRef};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[cfg(feature = "async-job")]
use super::AsyncEnginePoller;
use super::SslIoWrapper;

pub struct SslStream<S> {
    inner: ssl::SslStream<SslIoWrapper<S>>,
    #[cfg(feature = "async-job")]
    async_engine: Option<AsyncEnginePoller>,
}

impl<S> SslStream<S> {
    #[cfg(not(feature = "async-job"))]
    pub(crate) fn new(inner: ssl::SslStream<SslIoWrapper<S>>) -> Self {
        SslStream { inner }
    }

    #[cfg(feature = "async-job")]
    pub(crate) fn new(
        inner: ssl::SslStream<SslIoWrapper<S>>,
        async_engine: Option<AsyncEnginePoller>,
    ) -> Self {
        SslStream {
            inner,
            async_engine,
        }
    }

    #[inline]
    pub fn ssl(&self) -> &SslRef {
        self.inner.ssl()
    }

    #[inline]
    pub fn ssl_mut(&mut self) -> &mut SslRef {
        self.inner.ssl_mut()
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut S {
        self.inner.get_mut().get_mut()
    }

    fn set_cx(&mut self, cx: &mut Context<'_>) {
        self.inner.get_mut().set_cx(cx);
        #[cfg(all(feature = "async-job", ossl300))]
        if let Some(async_engine) = &self.async_engine {
            async_engine.set_cx(cx);
        }
    }

    #[cfg(feature = "async-job")]
    fn poll_async_engine(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Some(async_engine) = &mut self.async_engine {
            async_engine.poll_ready(self.inner.ssl(), cx)
        } else {
            Poll::Ready(Err(io::Error::other("async engine poller is not set")))
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslStream<S> {
    fn poll_read_unpin(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.set_cx(cx);

        loop {
            match self.inner.ssl_read_uninit(unsafe { buf.unfilled_mut() }) {
                Ok(n) => {
                    unsafe { buf.assume_init(n) };
                    buf.advance(n);
                    return Poll::Ready(Ok(()));
                }
                Err(e) => match e.code() {
                    ErrorCode::ZERO_RETURN => return Poll::Ready(Ok(())),
                    ErrorCode::WANT_READ => {
                        if e.io_error().is_none() {
                            continue;
                        } else {
                            return Poll::Pending;
                        }
                    }
                    ErrorCode::WANT_WRITE => return Poll::Pending,
                    #[cfg(feature = "async-job")]
                    ErrorCode::WANT_ASYNC => ready!(self.poll_async_engine(cx))?,
                    #[cfg(feature = "async-job")]
                    ErrorCode::WANT_ASYNC_JOB => {
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

    fn poll_write_unpin(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.set_cx(cx);

        loop {
            match self.inner.ssl_write(buf) {
                Ok(n) => return Poll::Ready(Ok(n)),
                Err(e) => match e.code() {
                    ErrorCode::WANT_READ => {
                        if e.io_error().is_none() {
                            continue;
                        } else {
                            return Poll::Pending;
                        }
                    }
                    ErrorCode::WANT_WRITE => return Poll::Pending,
                    #[cfg(feature = "async-job")]
                    ErrorCode::WANT_ASYNC => ready!(self.poll_async_engine(cx))?,
                    #[cfg(feature = "async-job")]
                    ErrorCode::WANT_ASYNC_JOB => {
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

    #[cfg(not(feature = "async-job"))]
    fn poll_shutdown_unpin(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.set_cx(cx);

        if let Err(e) = self.inner.shutdown() {
            match e.code() {
                ErrorCode::ZERO_RETURN => {}
                ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => return Poll::Pending,
                _ => {
                    return Poll::Ready(Err(e.into_io_error().unwrap_or_else(io::Error::other)));
                }
            }
        }

        self.inner.get_mut().get_pin_mut().poll_shutdown(cx)
    }

    #[cfg(feature = "async-job")]
    fn poll_shutdown_unpin(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.set_cx(cx);

        while let Err(e) = self.inner.shutdown() {
            match e.code() {
                ErrorCode::ZERO_RETURN => break,
                ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => return Poll::Pending,
                ErrorCode::WANT_ASYNC => ready!(self.poll_async_engine(cx))?,
                ErrorCode::WANT_ASYNC_JOB => {
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

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncRead for SslStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.as_mut().get_mut().poll_read_unpin(cx, buf)
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncWrite for SslStream<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.as_mut().get_mut().poll_write_unpin(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.inner.get_mut().get_pin_mut().poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.as_mut().get_mut().poll_shutdown_unpin(cx)
    }
}
