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
use std::sync::Arc;
use std::task::{Context, Poll};

use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use super::limited_read::{ArcLimitedReaderStats, LimitedReaderState, LimitedReaderStats};
use super::limited_write::{ArcLimitedWriterStats, LimitedWriterState, LimitedWriterStats};

#[pin_project]
pub struct LimitedStream<S> {
    #[pin]
    inner: S,
    reader_state: LimitedReaderState,
    writer_state: LimitedWriterState,
}

impl<S> LimitedStream<S> {
    pub fn new<ST>(
        inner: S,
        shift_millis: u8,
        read_max_bytes: usize,
        write_max_bytes: usize,
        stats: Arc<ST>,
    ) -> Self
    where
        ST: LimitedReaderStats + LimitedWriterStats + Send + Sync + 'static,
    {
        LimitedStream {
            inner,
            reader_state: LimitedReaderState::new(
                shift_millis,
                read_max_bytes,
                stats.clone() as ArcLimitedReaderStats,
            ),
            writer_state: LimitedWriterState::new(
                shift_millis,
                write_max_bytes,
                stats as ArcLimitedWriterStats,
            ),
        }
    }

    pub fn new_unlimited<ST>(inner: S, stats: Arc<ST>) -> Self
    where
        ST: LimitedReaderStats + LimitedWriterStats + Send + Sync + 'static,
    {
        LimitedStream {
            inner,
            reader_state: LimitedReaderState::new_unlimited(stats.clone() as ArcLimitedReaderStats),
            writer_state: LimitedWriterState::new_unlimited(stats as ArcLimitedWriterStats),
        }
    }

    pub fn reset_stats<ST>(&mut self, stats: Arc<ST>)
    where
        ST: LimitedReaderStats + LimitedWriterStats + Send + Sync + 'static,
    {
        self.reader_state
            .reset_stats(stats.clone() as ArcLimitedReaderStats);
        self.writer_state
            .reset_stats(stats as ArcLimitedWriterStats);
    }

    pub fn reset_limit(&mut self, shift_millis: u8, read_max_bytes: usize, write_max_bytes: usize) {
        self.reader_state.reset_limit(shift_millis, read_max_bytes);
        self.writer_state.reset_limit(shift_millis, write_max_bytes);
    }

    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<R: AsyncRead> AsyncRead for LimitedStream<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();
        this.reader_state.poll_read(this.inner, cx, buf)
    }
}

impl<W: AsyncWrite> AsyncWrite for LimitedStream<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        this.writer_state.poll_write(this.inner, cx, buf)
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().inner.poll_flush(cx)
    }

    #[inline]
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().inner.poll_shutdown(cx)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        if self.writer_state.limit_is_set() {
            let buf = bufs
                .iter()
                .find(|b| !b.is_empty())
                .map_or(&[][..], |b| &**b);
            self.poll_write(cx, buf)
        } else {
            self.project().inner.poll_write_vectored(cx, bufs)
        }
    }

    fn is_write_vectored(&self) -> bool {
        if self.writer_state.limit_is_set() {
            false
        } else {
            self.inner.is_write_vectored()
        }
    }
}
