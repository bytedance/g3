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
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use std::time::Duration;

use futures_util::FutureExt;
use tokio::time::{Instant, Sleep};

use crate::limit::{DatagramLimitInfo, DatagramLimitResult};
use crate::ArcLimitedSendStats;

pub trait AsyncUdpSend {
    fn poll_send_to(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        target: SocketAddr,
    ) -> Poll<io::Result<usize>>;

    fn poll_send(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>>;
}

pub struct LimitedUdpSend<T> {
    inner: T,
    delay: Pin<Box<Sleep>>,
    started: Instant,
    limit: DatagramLimitInfo,
    stats: ArcLimitedSendStats,
}

impl<T: AsyncUdpSend> LimitedUdpSend<T> {
    pub fn new(
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
            limit: DatagramLimitInfo::new(shift_millis, max_packets, max_bytes),
            stats,
        }
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
                DatagramLimitResult::Advance => {
                    let nw = ready!(self.inner.poll_send_to(cx, buf, target))?;
                    self.limit.set_advance(1, nw);
                    self.stats.add_send_packet();
                    self.stats.add_send_bytes(nw);
                    Poll::Ready(Ok(nw))
                }
                DatagramLimitResult::DelayFor(ms) => {
                    self.delay
                        .as_mut()
                        .reset(self.started + Duration::from_millis(dur_millis + ms));
                    self.delay.poll_unpin(cx).map(|_| Ok(0))
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
                DatagramLimitResult::Advance => {
                    let nw = ready!(self.inner.poll_send(cx, buf))?;
                    self.limit.set_advance(1, nw);
                    self.stats.add_send_packet();
                    self.stats.add_send_bytes(nw);
                    Poll::Ready(Ok(nw))
                }
                DatagramLimitResult::DelayFor(ms) => {
                    self.delay
                        .as_mut()
                        .reset(self.started + Duration::from_millis(dur_millis + ms));
                    self.delay.poll_unpin(cx).map(|_| Ok(0))
                }
            }
        } else {
            let nw = ready!(self.inner.poll_send(cx, buf))?;
            self.stats.add_send_packet();
            self.stats.add_send_bytes(nw);
            Poll::Ready(Ok(nw))
        }
    }
}
