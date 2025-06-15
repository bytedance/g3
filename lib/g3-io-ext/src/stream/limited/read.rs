/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::io::IoSlice;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, ready};
use std::time::Duration;

use futures_util::FutureExt;
use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::{Instant, Sleep};

use crate::limit::{GlobalLimitGroup, GlobalStreamLimit, StreamLimitAction, StreamLimiter};
use crate::stream::AsyncStream;

pub trait LimitedReaderStats {
    fn add_read_bytes(&self, size: usize);
}
pub type ArcLimitedReaderStats = Arc<dyn LimitedReaderStats + Send + Sync>;

#[derive(Default)]
pub struct NilLimitedReaderStats(());

impl LimitedReaderStats for NilLimitedReaderStats {
    fn add_read_bytes(&self, _size: usize) {}
}

pub(crate) struct LimitedReaderState {
    delay: Pin<Box<Sleep>>,
    started: Instant,
    limit: StreamLimiter,
    stats: ArcLimitedReaderStats,
}

impl LimitedReaderState {
    pub(crate) fn new(stats: ArcLimitedReaderStats) -> Self {
        LimitedReaderState {
            delay: Box::pin(tokio::time::sleep(Duration::from_millis(0))),
            started: Instant::now(),
            limit: StreamLimiter::default(),
            stats,
        }
    }

    pub(crate) fn local_limited(
        shift_millis: u8,
        max_bytes: usize,
        stats: ArcLimitedReaderStats,
    ) -> Self {
        LimitedReaderState {
            delay: Box::pin(tokio::time::sleep(Duration::from_millis(0))),
            started: Instant::now(),
            limit: StreamLimiter::with_local(shift_millis, max_bytes),
            stats,
        }
    }

    pub(crate) fn add_global_limiter<T>(&mut self, limiter: Arc<T>)
    where
        T: GlobalStreamLimit + Send + Sync + 'static,
    {
        self.limit.add_global(limiter);
    }

    #[inline]
    pub(crate) fn retain_global_limiter_by_group(&mut self, group: GlobalLimitGroup) {
        self.limit.retain_global_by_group(group);
    }

    pub(crate) fn reset_stats(&mut self, stats: ArcLimitedReaderStats) {
        self.stats = stats;
    }

    pub(crate) fn reset_local_limit(&mut self, shift_millis: u8, max_bytes: usize) {
        let dur_millis = self.started.elapsed().as_millis() as u64;
        self.limit.reset_local(shift_millis, max_bytes, dur_millis);
    }

    pub(crate) fn poll_read<R>(
        &mut self,
        reader: Pin<&mut R>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>>
    where
        R: AsyncRead,
    {
        if self.limit.is_set() {
            let dur_millis = self.started.elapsed().as_millis() as u64;
            match self.limit.check(dur_millis, buf.remaining()) {
                StreamLimitAction::AdvanceBy(len) => {
                    let mut limited_buf = ReadBuf::new(buf.initialize_unfilled_to(len));
                    match reader.poll_read(cx, &mut limited_buf) {
                        Poll::Ready(Ok(_)) => {
                            let nr = limited_buf.filled().len();
                            self.limit.set_advance(nr);
                            buf.advance(nr);
                            self.stats.add_read_bytes(nr);
                            Poll::Ready(Ok(()))
                        }
                        Poll::Ready(Err(e)) => {
                            self.limit.release_global();
                            Poll::Ready(Err(e))
                        }
                        Poll::Pending => {
                            self.limit.release_global();
                            Poll::Pending
                        }
                    }
                }
                StreamLimitAction::DelayUntil(t) => {
                    self.delay.as_mut().reset(t);
                    match self.delay.poll_unpin(cx) {
                        Poll::Ready(_) => {
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        }
                        Poll::Pending => Poll::Pending,
                    }
                }
                StreamLimitAction::DelayFor(ms) => {
                    self.delay
                        .as_mut()
                        .reset(self.started + Duration::from_millis(dur_millis + ms));
                    match self.delay.poll_unpin(cx) {
                        Poll::Ready(_) => {
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        }
                        Poll::Pending => Poll::Pending,
                    }
                }
            }
        } else {
            let old_filled_len = buf.filled().len();
            ready!(reader.poll_read(cx, buf))?;
            let nr = buf.filled().len() - old_filled_len;
            self.stats.add_read_bytes(nr);
            Poll::Ready(Ok(()))
        }
    }
}

pin_project! {
    pub struct LimitedReader<R> {
        #[pin]
        inner: R,
        state: LimitedReaderState,
    }
}

impl<R> LimitedReader<R> {
    pub fn new(inner: R, stats: ArcLimitedReaderStats) -> Self {
        LimitedReader {
            inner,
            state: LimitedReaderState::new(stats),
        }
    }

    pub fn local_limited(
        inner: R,
        shift_millis: u8,
        max_bytes: usize,
        stats: ArcLimitedReaderStats,
    ) -> Self {
        LimitedReader {
            inner,
            state: LimitedReaderState::local_limited(shift_millis, max_bytes, stats),
        }
    }

    pub fn add_global_limiter<T>(&mut self, limiter: Arc<T>)
    where
        T: GlobalStreamLimit + Send + Sync + 'static,
    {
        self.state.add_global_limiter(limiter);
    }

    #[inline]
    pub fn retain_global_limiter_by_group(&mut self, group: GlobalLimitGroup) {
        self.state.retain_global_limiter_by_group(group);
    }

    pub(crate) fn from_parts(inner: R, state: LimitedReaderState) -> Self {
        LimitedReader { inner, state }
    }

    #[inline]
    pub fn reset_stats(&mut self, stats: ArcLimitedReaderStats) {
        self.state.reset_stats(stats);
    }

    #[inline]
    pub fn reset_local_limit(&mut self, shift_millis: u8, max_bytes: usize) {
        self.state.reset_local_limit(shift_millis, max_bytes);
    }

    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R> AsyncRead for LimitedReader<R>
where
    R: AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();
        this.state.poll_read(this.inner, cx, buf)
    }
}

impl<S: AsyncRead + AsyncWrite> AsyncWrite for LimitedReader<S> {
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

impl<S> AsyncStream for LimitedReader<S>
where
    S: AsyncStream,
    S::R: AsyncRead,
    S::W: AsyncWrite,
{
    type R = LimitedReader<S::R>;
    type W = S::W;

    fn into_split(self) -> (Self::R, Self::W) {
        let (r, w) = self.inner.into_split();
        (
            LimitedReader {
                inner: r,
                state: self.state,
            },
            w,
        )
    }
}

pin_project! {
    pub struct SizedReader<R> {
        #[pin]
        inner: R,
        max_size: u64,
        cur_size: u64,
    }
}

impl<R> SizedReader<R>
where
    R: AsyncRead,
{
    pub fn new(inner: R, max_size: u64) -> Self {
        SizedReader {
            inner,
            max_size,
            cur_size: 0,
        }
    }
}

impl<R> AsyncRead for SizedReader<R>
where
    R: AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.cur_size >= self.max_size {
            return Poll::Ready(Ok(()));
        }

        let len = buf
            .remaining()
            .min((self.max_size - self.cur_size).min(usize::MAX as u64) as usize);
        let mut limited_buf = ReadBuf::new(buf.initialize_unfilled_to(len));

        let this = self.project();
        ready!(this.inner.poll_read(cx, &mut limited_buf))?;
        let nr = limited_buf.filled().len();
        buf.advance(nr);
        Poll::Ready(Ok(()))
    }
}
