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
use std::task::{ready, Context, Poll};

use pin_project::pin_project;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, ReadBuf};

use super::DEFAULT_BUF_SIZE;
use crate::io::{ArcLimitedReaderStats, LimitedReader};

#[pin_project]
pub struct LimitedBufReader<R> {
    #[pin]
    inner: LimitedReader<R>,
    stats: ArcLimitedReaderStats,
    buf: Box<[u8]>,
    pos: usize,
    cap: usize,
}

impl<R> LimitedBufReader<R>
where
    R: AsyncRead,
{
    pub fn new(
        inner: R,
        shift_millis: u8,
        max_bytes: usize,
        direct_stats: ArcLimitedReaderStats,
        buffer_stats: ArcLimitedReaderStats,
    ) -> Self {
        LimitedBufReader::with_capacity(
            DEFAULT_BUF_SIZE,
            inner,
            shift_millis,
            max_bytes,
            direct_stats,
            buffer_stats,
        )
    }

    pub fn new_directed(from: LimitedReader<R>, buffer_stats: ArcLimitedReaderStats) -> Self {
        LimitedBufReader::directly_with_capacity(DEFAULT_BUF_SIZE, from, buffer_stats)
    }

    pub fn new_unlimited(
        inner: R,
        direct_stats: ArcLimitedReaderStats,
        buffer_stats: ArcLimitedReaderStats,
    ) -> Self {
        LimitedBufReader::unlimited_with_capacity(
            DEFAULT_BUF_SIZE,
            inner,
            direct_stats,
            buffer_stats,
        )
    }

    pub fn with_capacity(
        capacity: usize,
        inner: R,
        shift_millis: u8,
        max_bytes: usize,
        direct_stats: ArcLimitedReaderStats,
        buffer_stats: ArcLimitedReaderStats,
    ) -> Self {
        let buffer = vec![0; capacity];
        LimitedBufReader {
            inner: LimitedReader::new(inner, shift_millis, max_bytes, direct_stats),
            stats: buffer_stats,
            buf: buffer.into_boxed_slice(),
            pos: 0,
            cap: 0,
        }
    }

    pub fn directly_with_capacity(
        capacity: usize,
        from: LimitedReader<R>,
        buffer_stats: ArcLimitedReaderStats,
    ) -> Self {
        let buffer = vec![0; capacity];
        LimitedBufReader {
            inner: from,
            stats: buffer_stats,
            buf: buffer.into_boxed_slice(),
            pos: 0,
            cap: 0,
        }
    }

    pub fn unlimited_with_capacity(
        capacity: usize,
        inner: R,
        direct_stats: ArcLimitedReaderStats,
        buffer_stats: ArcLimitedReaderStats,
    ) -> Self {
        let buffer = vec![0; capacity];
        LimitedBufReader {
            inner: LimitedReader::new_unlimited(inner, direct_stats),
            stats: buffer_stats,
            buf: buffer.into_boxed_slice(),
            pos: 0,
            cap: 0,
        }
    }

    pub fn reset_direct_stats(&mut self, stats: ArcLimitedReaderStats) {
        self.inner.reset_stats(stats);
    }

    pub fn reset_buffer_stats(&mut self, stats: ArcLimitedReaderStats) {
        self.stats = stats;
    }

    pub fn reset_limit(&mut self, shift_millis: u8, max_bytes: usize) {
        self.inner.reset_limit(shift_millis, max_bytes);
    }

    pub fn into_inner(self) -> R {
        self.inner.into_inner()
    }

    fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut LimitedReader<R>> {
        self.project().inner
    }
}

impl<R: AsyncRead> AsyncRead for LimitedBufReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // If we don't have any buffered data and we're doing a massive read
        // (larger than our internal buffer), bypass our internal buffer
        // entirely.
        if self.pos == self.cap && buf.remaining() >= self.buf.len() {
            let old_filled_len = buf.filled().len();
            let res = ready!(self.as_mut().get_pin_mut().poll_read(cx, buf));
            let nr = buf.filled().len() - old_filled_len;
            self.stats.add_read_bytes(nr);
            Poll::Ready(res)
        } else {
            let rem = ready!(self.as_mut().poll_fill_buf(cx))?;
            let amt = std::cmp::min(rem.len(), buf.remaining());
            buf.put_slice(&rem[..amt]);
            self.consume(amt);
            Poll::Ready(Ok(()))
        }
    }
}

impl<R: AsyncRead> AsyncBufRead for LimitedBufReader<R> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        let me = self.project();
        if *me.pos >= *me.cap {
            let mut buf = ReadBuf::new(me.buf);
            ready!(me.inner.poll_read(cx, &mut buf))?;
            *me.cap = buf.filled().len();
            *me.pos = 0;
        }

        Poll::Ready(Ok(&me.buf[*me.pos..*me.cap]))
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let new_pos = std::cmp::min(self.pos + amt, self.cap);
        self.stats.add_read_bytes(new_pos - self.pos);
        let me = self.project();
        *me.pos = new_pos;
    }
}

impl<R: AsyncRead + AsyncWrite> AsyncWrite for LimitedBufReader<R> {
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
