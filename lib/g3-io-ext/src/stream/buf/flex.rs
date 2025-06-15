/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::io::IoSlice;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use bytes::{Buf, Bytes, BytesMut};
use pin_project_lite::pin_project;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, ReadBuf};

use super::{DEFAULT_BUF_SIZE, OnceBufReader};
use crate::stream::AsyncStream;

pin_project! {
    pub struct FlexBufReader<R> {
        #[pin]
        inner: R,
        buf: Box<[u8]>,
        pos: usize,
        cap: usize,
    }
}

impl<R> FlexBufReader<R> {
    /// Creates a new `BufReader` with a default buffer capacity. The default is currently 8 KB,
    /// but may change in the future.
    pub fn new(inner: R) -> Self {
        Self::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    /// Creates a new `BufReader` with the specified buffer capacity.
    pub fn with_capacity(capacity: usize, inner: R) -> Self {
        Self::with_buffer(vec![0; capacity], 0, inner)
    }

    /// Creates a new `BufReader` with BytesMut
    pub fn with_bytes(bytes: BytesMut, inner: R) -> Self {
        let vec: Vec<u8> = bytes.into();
        let len = vec.len();
        Self::with_buffer(vec, len, inner)
    }

    /// Creates a new `BufReader` with a existed buffer
    pub fn with_buffer(mut buffer: Vec<u8>, len: usize, inner: R) -> Self {
        unsafe {
            // safe here as we didn't use the uninitialized data
            buffer.set_len(buffer.capacity())
        };
        Self {
            inner,
            buf: buffer.into_boxed_slice(),
            pos: 0,
            cap: len,
        }
    }

    /// Gets a reference to the underlying reader.
    ///
    /// It is inadvisable to directly read from the underlying reader.
    fn get_ref(&self) -> &R {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Gets a pinned mutable reference to the underlying reader.
    ///
    /// It is inadvisable to directly read from the underlying reader.
    fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut R> {
        self.project().inner
    }

    /// Consumes this `BufReader`, returning the underlying reader.
    ///
    /// Note that any leftover data in the internal buffer is lost.
    pub fn into_inner(self) -> R {
        self.inner
    }

    pub fn into_parts(self) -> (Bytes, R) {
        if self.pos < self.cap {
            let mut bytes = Bytes::from(self.buf);
            let _ = bytes.split_off(self.cap);
            bytes.advance(self.pos);
            (bytes, self.inner)
        } else {
            (Bytes::new(), self.inner)
        }
    }

    /// Returns a reference to the internally buffered data.
    ///
    /// Unlike `fill_buf`, this will not attempt to fill the buffer if it is empty.
    pub fn buffer(&self) -> &[u8] {
        &self.buf[self.pos..self.cap]
    }

    /// Invalidates all data in the internal buffer.
    #[inline]
    fn discard_buffer(self: Pin<&mut Self>) {
        let me = self.project();
        *me.pos = 0;
        *me.cap = 0;
    }
}

impl<R: AsyncRead> AsyncRead for FlexBufReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // If we don't have any buffered data and we're doing a massive read
        // (larger than our internal buffer), bypass our internal buffer
        // entirely.
        if self.pos == self.cap && buf.remaining() >= self.buf.len() {
            let res = ready!(self.as_mut().get_pin_mut().poll_read(cx, buf));
            self.discard_buffer();
            return Poll::Ready(res);
        }
        let rem = ready!(self.as_mut().poll_fill_buf(cx))?;
        let amt = std::cmp::min(rem.len(), buf.remaining());
        buf.put_slice(&rem[..amt]);
        self.consume(amt);
        Poll::Ready(Ok(()))
    }
}

impl<R: AsyncRead> AsyncBufRead for FlexBufReader<R> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        let me = self.project();

        // If we've reached the end of our internal buffer then we need to fetch
        // some more data from the underlying reader.
        // Branch using `>=` instead of the more correct `==`
        // to tell the compiler that the pos..cap slice is always valid.
        if *me.pos >= *me.cap {
            debug_assert!(*me.pos == *me.cap);
            let mut buf = ReadBuf::new(me.buf);
            ready!(me.inner.poll_read(cx, &mut buf))?;
            *me.cap = buf.filled().len();
            *me.pos = 0;
        }
        Poll::Ready(Ok(&me.buf[*me.pos..*me.cap]))
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let me = self.project();
        *me.pos = std::cmp::min(*me.pos + amt, *me.cap);
    }
}

impl<S: AsyncRead + AsyncWrite> AsyncWrite for FlexBufReader<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.get_pin_mut().poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.get_pin_mut().poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.get_pin_mut().poll_shutdown(cx)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.get_pin_mut().poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.get_ref().is_write_vectored()
    }
}

impl<S> AsyncStream for FlexBufReader<S>
where
    S: AsyncStream,
    S::R: AsyncRead,
    S::W: AsyncWrite,
{
    type R = FlexBufReader<S::R>;
    type W = S::W;

    fn into_split(self) -> (Self::R, Self::W) {
        let (r, w) = self.inner.into_split();
        (
            FlexBufReader {
                inner: r,
                buf: self.buf,
                pos: self.pos,
                cap: self.cap,
            },
            w,
        )
    }
}

impl<S> From<FlexBufReader<S>> for OnceBufReader<S> {
    fn from(value: FlexBufReader<S>) -> Self {
        let (buf, stream) = value.into_parts();
        OnceBufReader::with_bytes(stream, buf)
    }
}

#[cfg(test)]
mod tests {
    use super::FlexBufReader;
    use bytes::{BufMut, BytesMut};
    use tokio::io::AsyncBufReadExt;

    #[tokio::test]
    async fn with_bytes() {
        let mut b = BytesMut::with_capacity(12);
        let buf_content = b"1234";
        b.put_slice(buf_content);

        let content = b"test message";
        let stream = tokio_test::io::Builder::new().read(content).build();

        let mut v = FlexBufReader::with_bytes(b, stream);
        let buf = v.buffer();
        assert_eq!(buf, buf_content);
        v.consume(4);
        assert!(v.buffer().is_empty());

        let buf = v.fill_buf().await.unwrap();
        assert_eq!(buf, content);
    }
}
