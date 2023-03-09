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
use std::io::IoSlice;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Buf, Bytes, BytesMut};
use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[pin_project]
pub struct OnceBufReader<R: AsyncRead> {
    #[pin]
    inner: R,
    buf: Option<Bytes>,
}

impl<R: AsyncRead> OnceBufReader<R> {
    pub fn new(inner: R, buf: BytesMut) -> Self {
        if buf.is_empty() {
            OnceBufReader { inner, buf: None }
        } else {
            OnceBufReader {
                inner,
                buf: Some(buf.freeze()),
            }
        }
    }

    pub fn with_no_buf(inner: R) -> Self {
        OnceBufReader { inner, buf: None }
    }

    pub fn take_buf(&mut self) -> Option<Bytes> {
        self.buf.take()
    }

    pub fn buf(&self) -> Option<&Bytes> {
        self.buf.as_ref()
    }

    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: AsyncRead> AsyncRead for OnceBufReader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();

        if let Some(mut cache) = this.buf.take() {
            let to_read = buf.remaining().min(cache.len());
            buf.put_slice(&cache.chunk()[0..to_read]);
            cache.advance(to_read);
            if cache.is_empty() {
                *this.buf = None;
            } else {
                *this.buf = Some(cache);
            }
            Poll::Ready(Ok(()))
        } else {
            this.inner.poll_read(cx, buf)
        }
    }
}

impl<R: AsyncRead + AsyncWrite> AsyncWrite for OnceBufReader<R> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.project().inner.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().inner.poll_shutdown(cx)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.project().inner.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
}
