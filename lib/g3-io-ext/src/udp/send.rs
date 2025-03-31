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

use std::io::{self, IoSlice};
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, ready};
use std::time::Duration;

use futures_util::FutureExt;
use tokio::time::{Instant, Sleep};

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "macos",
    target_os = "solaris",
))]
use super::SendMsgHdr;
use crate::limit::{DatagramLimitAction, DatagramLimiter};
use crate::{ArcLimitedSendStats, GlobalDatagramLimit};

pub trait AsyncUdpSend {
    fn poll_send_to(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        target: SocketAddr,
    ) -> Poll<io::Result<usize>>;

    fn poll_send(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>>;

    fn poll_sendmsg(
        &mut self,
        cx: &mut Context<'_>,
        iov: &[IoSlice<'_>],
        target: Option<SocketAddr>,
    ) -> Poll<io::Result<usize>>;

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    fn poll_batch_sendmsg<const C: usize>(
        &mut self,
        cx: &mut Context<'_>,
        msgs: &mut [SendMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>>;

    #[cfg(target_os = "macos")]
    fn poll_batch_sendmsg_x<const C: usize>(
        &mut self,
        cx: &mut Context<'_>,
        msgs: &mut [SendMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>>;
}

pub struct LimitedUdpSend<T> {
    inner: T,
    delay: Pin<Box<Sleep>>,
    started: Instant,
    limit: DatagramLimiter,
    stats: ArcLimitedSendStats,
}

impl<T: AsyncUdpSend> LimitedUdpSend<T> {
    pub fn local_limited(
        inner: T,
        shift_millis: u8,
        max_packets: usize,
        max_bytes: usize,
        stats: ArcLimitedSendStats,
    ) -> Self {
        LimitedUdpSend {
            inner,
            delay: Box::pin(tokio::time::sleep(Duration::from_millis(0))),
            started: Instant::now(),
            limit: DatagramLimiter::with_local(shift_millis, max_packets, max_bytes),
            stats,
        }
    }

    #[inline]
    pub fn add_global_limiter<L>(&mut self, limiter: Arc<L>)
    where
        L: GlobalDatagramLimit + Send + Sync + 'static,
    {
        self.limit.add_global(limiter);
    }

    pub fn reset_stats(&mut self, stats: ArcLimitedSendStats) {
        self.stats = stats;
    }
}

impl<T> AsyncUdpSend for LimitedUdpSend<T>
where
    T: AsyncUdpSend + Send,
{
    fn poll_send_to(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        target: SocketAddr,
    ) -> Poll<io::Result<usize>> {
        if self.limit.is_set() {
            let dur_millis = self.started.elapsed().as_millis() as u64;
            match self.limit.check_packet(dur_millis, buf.len()) {
                DatagramLimitAction::Advance(_) => match self.inner.poll_send_to(cx, buf, target) {
                    Poll::Ready(Ok(nw)) => {
                        self.limit.set_advance(1, nw);
                        self.stats.add_send_packet();
                        self.stats.add_send_bytes(nw);
                        Poll::Ready(Ok(nw))
                    }
                    Poll::Ready(Err(e)) => {
                        self.limit.release_global();
                        Poll::Ready(Err(e))
                    }
                    Poll::Pending => {
                        self.limit.release_global();
                        Poll::Pending
                    }
                },
                DatagramLimitAction::DelayUntil(t) => {
                    self.delay.as_mut().reset(t);
                    match self.delay.poll_unpin(cx) {
                        Poll::Ready(_) => {
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        }
                        Poll::Pending => Poll::Pending,
                    }
                }
                DatagramLimitAction::DelayFor(ms) => {
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
            let nw = ready!(self.inner.poll_send_to(cx, buf, target))?;
            self.stats.add_send_packet();
            self.stats.add_send_bytes(nw);
            Poll::Ready(Ok(nw))
        }
    }

    fn poll_send(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        if self.limit.is_set() {
            let dur_millis = self.started.elapsed().as_millis() as u64;
            match self.limit.check_packet(dur_millis, buf.len()) {
                DatagramLimitAction::Advance(_) => match self.inner.poll_send(cx, buf) {
                    Poll::Ready(Ok(nw)) => {
                        self.limit.set_advance(1, nw);
                        self.stats.add_send_packet();
                        self.stats.add_send_bytes(nw);
                        Poll::Ready(Ok(nw))
                    }
                    Poll::Ready(Err(e)) => {
                        self.limit.release_global();
                        Poll::Ready(Err(e))
                    }
                    Poll::Pending => {
                        self.limit.release_global();
                        Poll::Pending
                    }
                },
                DatagramLimitAction::DelayUntil(t) => {
                    self.delay.as_mut().reset(t);
                    match self.delay.poll_unpin(cx) {
                        Poll::Ready(_) => {
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        }
                        Poll::Pending => Poll::Pending,
                    }
                }
                DatagramLimitAction::DelayFor(ms) => {
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
            let nw = ready!(self.inner.poll_send(cx, buf))?;
            self.stats.add_send_packet();
            self.stats.add_send_bytes(nw);
            Poll::Ready(Ok(nw))
        }
    }

    fn poll_sendmsg(
        &mut self,
        cx: &mut Context<'_>,
        iov: &[IoSlice<'_>],
        target: Option<SocketAddr>,
    ) -> Poll<io::Result<usize>> {
        if self.limit.is_set() {
            let dur_millis = self.started.elapsed().as_millis() as u64;
            let len = iov.iter().map(|v| v.len()).sum();
            match self.limit.check_packet(dur_millis, len) {
                DatagramLimitAction::Advance(_) => match self.inner.poll_sendmsg(cx, iov, target) {
                    Poll::Ready(Ok(nw)) => {
                        self.limit.set_advance(1, nw);
                        self.stats.add_send_packet();
                        self.stats.add_send_bytes(nw);
                        Poll::Ready(Ok(nw))
                    }
                    Poll::Ready(Err(e)) => {
                        self.limit.release_global();
                        Poll::Ready(Err(e))
                    }
                    Poll::Pending => {
                        self.limit.release_global();
                        Poll::Pending
                    }
                },
                DatagramLimitAction::DelayUntil(t) => {
                    self.delay.as_mut().reset(t);
                    match self.delay.poll_unpin(cx) {
                        Poll::Ready(_) => {
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        }
                        Poll::Pending => Poll::Pending,
                    }
                }
                DatagramLimitAction::DelayFor(ms) => {
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
            let nw = ready!(self.inner.poll_sendmsg(cx, iov, target))?;
            self.stats.add_send_packet();
            self.stats.add_send_bytes(nw);
            Poll::Ready(Ok(nw))
        }
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    fn poll_batch_sendmsg<const C: usize>(
        &mut self,
        cx: &mut Context<'_>,
        msgs: &mut [SendMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>> {
        use smallvec::SmallVec;

        if self.limit.is_set() {
            let dur_millis = self.started.elapsed().as_millis() as u64;
            let mut total_size_v = SmallVec::<[usize; 32]>::with_capacity(msgs.len());
            let mut total_size = 0;
            for msg in msgs.iter() {
                total_size += msg.iov.iter().map(|v| v.len()).sum::<usize>();
                total_size_v.push(total_size);
            }
            match self.limit.check_packets(dur_millis, total_size_v.as_ref()) {
                DatagramLimitAction::Advance(n) => {
                    match self.inner.poll_batch_sendmsg(cx, &mut msgs[0..n]) {
                        Poll::Ready(Ok(count)) => {
                            let len = msgs.iter().take(count).map(|v| v.n_send).sum();
                            self.limit.set_advance(count, len);
                            self.stats.add_send_packets(count);
                            self.stats.add_send_bytes(len);
                            Poll::Ready(Ok(count))
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
                DatagramLimitAction::DelayUntil(t) => {
                    self.delay.as_mut().reset(t);
                    match self.delay.poll_unpin(cx) {
                        Poll::Ready(_) => {
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        }
                        Poll::Pending => Poll::Pending,
                    }
                }
                DatagramLimitAction::DelayFor(ms) => {
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
            let count = ready!(self.inner.poll_batch_sendmsg(cx, msgs))?;
            self.stats.add_send_packets(count);
            self.stats
                .add_send_bytes(msgs.iter().take(count).map(|h| h.n_send).sum());
            Poll::Ready(Ok(count))
        }
    }

    #[cfg(target_os = "macos")]
    fn poll_batch_sendmsg_x<const C: usize>(
        &mut self,
        cx: &mut Context<'_>,
        msgs: &mut [SendMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>> {
        use smallvec::SmallVec;

        if self.limit.is_set() {
            let dur_millis = self.started.elapsed().as_millis() as u64;
            let mut total_size_v = SmallVec::<[usize; 32]>::with_capacity(msgs.len());
            let mut total_size = 0;
            for msg in msgs.iter() {
                total_size += msg.iov.iter().map(|v| v.len()).sum::<usize>();
                total_size_v.push(total_size);
            }
            match self.limit.check_packets(dur_millis, total_size_v.as_ref()) {
                DatagramLimitAction::Advance(n) => {
                    match self.inner.poll_batch_sendmsg_x(cx, &mut msgs[0..n]) {
                        Poll::Ready(Ok(count)) => {
                            let len = msgs.iter().take(count).map(|v| v.n_send).sum();
                            self.limit.set_advance(count, len);
                            self.stats.add_send_packets(count);
                            self.stats.add_send_bytes(len);
                            Poll::Ready(Ok(count))
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
                DatagramLimitAction::DelayUntil(t) => {
                    self.delay.as_mut().reset(t);
                    match self.delay.poll_unpin(cx) {
                        Poll::Ready(_) => {
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        }
                        Poll::Pending => Poll::Pending,
                    }
                }
                DatagramLimitAction::DelayFor(ms) => {
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
            let count = ready!(self.inner.poll_batch_sendmsg_x(cx, msgs))?;
            self.stats.add_send_packets(count);
            self.stats
                .add_send_bytes(msgs.iter().take(count).map(|h| h.n_send).sum());
            Poll::Ready(Ok(count))
        }
    }
}
