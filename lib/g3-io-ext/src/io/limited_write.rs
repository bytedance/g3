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
use std::task::{ready, Context, Poll};
use std::time::Duration;

use futures_util::FutureExt;
use pin_project::pin_project;
use tokio::io::AsyncWrite;
use tokio::time::{Instant, Sleep};

use crate::limit::{StreamLimitInfo, StreamLimitResult};

pub trait LimitedWriterStats {
    fn add_write_bytes(&self, size: usize);
}
pub type ArcLimitedWriterStats = Arc<dyn LimitedWriterStats + Send + Sync>;

#[derive(Default)]
pub struct NilLimitedWriterStats(());

impl LimitedWriterStats for NilLimitedWriterStats {
    fn add_write_bytes(&self, _size: usize) {}
}

pub(crate) struct LimitedWriterState {
    delay: Pin<Box<Sleep>>,
    started: Instant,
    limit: StreamLimitInfo,
    stats: ArcLimitedWriterStats,
}

impl LimitedWriterState {
    pub(crate) fn new(shift_millis: u8, max_bytes: usize, stats: ArcLimitedWriterStats) -> Self {
        LimitedWriterState {
            delay: Box::pin(tokio::time::sleep(Duration::from_millis(0))),
            started: Instant::now(),
            limit: StreamLimitInfo::new(shift_millis, max_bytes),
            stats,
        }
    }

    pub(crate) fn new_unlimited(stats: ArcLimitedWriterStats) -> Self {
        LimitedWriterState {
            delay: Box::pin(tokio::time::sleep(Duration::from_millis(0))),
            started: Instant::now(),
            limit: StreamLimitInfo::default(),
            stats,
        }
    }

    pub(crate) fn reset_stats(&mut self, stats: ArcLimitedWriterStats) {
        self.stats = stats;
    }

    pub(crate) fn reset_limit(&mut self, shift_millis: u8, max_bytes: usize) {
        let dur_millis = self.started.elapsed().as_millis() as u64;
        self.limit.reset(shift_millis, max_bytes, dur_millis);
    }

    #[inline]
    pub(crate) fn limit_is_set(&self) -> bool {
        self.limit.is_set()
    }

    pub(crate) fn poll_write<W>(
        &mut self,
        writer: Pin<&mut W>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>>
    where
        W: AsyncWrite,
    {
        if self.limit.is_set() {
            let dur_millis = self.started.elapsed().as_millis() as u64;
            match self.limit.check(dur_millis, buf.len()) {
                StreamLimitResult::AdvanceBy(len) => {
                    let nw = ready!(writer.poll_write(cx, &buf[..len]))?;
                    self.limit.set_advance(nw);
                    self.stats.add_write_bytes(nw);
                    Poll::Ready(Ok(nw))
                }
                StreamLimitResult::DelayFor(ms) => {
                    self.delay
                        .as_mut()
                        .reset(self.started + Duration::from_millis(dur_millis + ms));
                    self.delay.poll_unpin(cx).map(|_| Ok(0))
                }
            }
        } else {
            let nw = ready!(writer.poll_write(cx, buf))?;
            self.stats.add_write_bytes(nw);
            Poll::Ready(Ok(nw))
        }
    }
}

#[pin_project]
pub struct LimitedWriter<W> {
    #[pin]
    inner: W,
    state: LimitedWriterState,
}

impl<W: AsyncWrite> LimitedWriter<W> {
    pub fn new(inner: W, shift_millis: u8, max_bytes: usize, stats: ArcLimitedWriterStats) -> Self {
        LimitedWriter {
            inner,
            state: LimitedWriterState::new(shift_millis, max_bytes, stats),
        }
    }

    pub fn new_unlimited(inner: W, stats: ArcLimitedWriterStats) -> Self {
        LimitedWriter {
            inner,
            state: LimitedWriterState::new_unlimited(stats),
        }
    }

    #[inline]
    pub fn reset_stats(&mut self, stats: ArcLimitedWriterStats) {
        self.state.reset_stats(stats)
    }

    #[inline]
    pub fn reset_limit(&mut self, shift_millis: u8, max_bytes: usize) {
        self.state.reset_limit(shift_millis, max_bytes)
    }

    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: AsyncWrite> AsyncWrite for LimitedWriter<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        this.state.poll_write(this.inner, cx, buf)
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
        if self.state.limit_is_set() {
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
        if self.state.limit_is_set() {
            false
        } else {
            self.inner.is_write_vectored()
        }
    }
}
