/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::io::IoSlice;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use super::limited_read::{LimitedReaderState, LimitedReaderStats};
use super::limited_write::{LimitedWriterState, LimitedWriterStats};
use crate::limit::GlobalStreamLimit;
use crate::{AsyncStream, LimitedReader, LimitedWriter};

pin_project! {
    pub struct LimitedStream<S> {
        #[pin]
        inner: S,
        reader_state: LimitedReaderState,
        writer_state: LimitedWriterState,
    }
}

impl<S> LimitedStream<S> {
    pub fn new<ST>(inner: S, stats: Arc<ST>) -> Self
    where
        ST: LimitedReaderStats + LimitedWriterStats + Send + Sync + 'static,
    {
        LimitedStream {
            inner,
            reader_state: LimitedReaderState::new(stats.clone()),
            writer_state: LimitedWriterState::new(stats),
        }
    }

    pub fn local_limited<ST>(
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
            reader_state: LimitedReaderState::local_limited(
                shift_millis,
                read_max_bytes,
                stats.clone(),
            ),
            writer_state: LimitedWriterState::local_limited(shift_millis, write_max_bytes, stats),
        }
    }

    pub fn add_global_limiter<T>(&mut self, read_limiter: Arc<T>, write_limiter: Arc<T>)
    where
        T: GlobalStreamLimit + Send + Sync + 'static,
    {
        self.reader_state.add_global_limiter(read_limiter);
        self.writer_state.add_global_limiter(write_limiter);
    }

    pub fn reset_stats<ST>(&mut self, stats: Arc<ST>)
    where
        ST: LimitedReaderStats + LimitedWriterStats + Send + Sync + 'static,
    {
        self.reader_state.reset_stats(stats.clone());
        self.writer_state.reset_stats(stats);
    }

    pub fn reset_local_limit(
        &mut self,
        shift_millis: u8,
        read_max_bytes: usize,
        write_max_bytes: usize,
    ) {
        self.reader_state
            .reset_local_limit(shift_millis, read_max_bytes);
        self.writer_state
            .reset_local_limit(shift_millis, write_max_bytes);
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

impl<S> AsyncStream for LimitedStream<S>
where
    S: AsyncStream,
    S::R: AsyncRead,
    S::W: AsyncWrite,
{
    type R = LimitedReader<S::R>;
    type W = LimitedWriter<S::W>;

    fn into_split(self) -> (Self::R, Self::W) {
        let (r, w) = self.inner.into_split();
        (
            LimitedReader::from_parts(r, self.reader_state),
            LimitedWriter::from_parts(w, self.writer_state),
        )
    }
}
