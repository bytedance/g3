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

use std::cell::UnsafeCell;
use std::fmt;
use std::future::Future;
use std::io::{self, IoSliceMut};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{ready, Context, Poll};
use std::time::Duration;

use futures_util::FutureExt;
use quinn::udp;
use quinn::{AsyncTimer, AsyncUdpSocket, Runtime, TokioRuntime};
use tokio::time::{Instant, Sleep};

use crate::limit::{DatagramLimitInfo, DatagramLimitResult};
use crate::{ArcLimitedRecvStats, ArcLimitedSendStats, LimitedRecvStats, LimitedSendStats};

pub struct LimitedTokioRuntime<ST> {
    inner: TokioRuntime,
    shift_millis: u8,
    max_send_packets: usize,
    max_send_bytes: usize,
    max_recv_packets: usize,
    max_recv_bytes: usize,
    stats: Arc<ST>,
}

impl<ST> LimitedTokioRuntime<ST> {
    pub fn new(
        inner: TokioRuntime,
        shift_millis: u8,
        max_send_packets: usize,
        max_send_bytes: usize,
        max_recv_packets: usize,
        max_recv_bytes: usize,
        stats: Arc<ST>,
    ) -> Self {
        LimitedTokioRuntime {
            inner,
            shift_millis,
            max_send_packets,
            max_send_bytes,
            max_recv_packets,
            max_recv_bytes,
            stats,
        }
    }
}

impl<ST> fmt::Debug for LimitedTokioRuntime<ST> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<ST> Runtime for LimitedTokioRuntime<ST>
where
    ST: LimitedSendStats + LimitedRecvStats + Send + Sync + 'static,
{
    fn new_timer(&self, t: std::time::Instant) -> Pin<Box<dyn AsyncTimer>> {
        self.inner.new_timer(t)
    }

    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>) {
        self.inner.spawn(future);
    }

    fn wrap_udp_socket(&self, sock: std::net::UdpSocket) -> io::Result<Box<dyn AsyncUdpSocket>> {
        let inner = self.inner.wrap_udp_socket(sock)?;
        Ok(Box::new(LimitedUdpSocket::new(
            inner,
            self.shift_millis,
            self.max_send_packets,
            self.max_send_bytes,
            self.max_recv_packets,
            self.max_recv_bytes,
            self.stats.clone(),
        )))
    }
}

struct LimitedSendState {
    delay: Pin<Box<Sleep>>,
    started: Instant,
    limit: DatagramLimitInfo,
    stats: ArcLimitedSendStats,
}

impl LimitedSendState {
    fn new(
        started: Instant,
        shift_millis: u8,
        max_packets: usize,
        max_bytes: usize,
        stats: ArcLimitedSendStats,
    ) -> Self {
        LimitedSendState {
            delay: Box::pin(tokio::time::sleep(Duration::from_millis(0))),
            started,
            limit: DatagramLimitInfo::new(shift_millis, max_packets, max_bytes),
            stats,
        }
    }
}

struct LimitedRecvState {
    delay: Pin<Box<Sleep>>,
    started: Instant,
    limit: DatagramLimitInfo,
    stats: ArcLimitedRecvStats,
}

impl LimitedRecvState {
    fn new(
        started: Instant,
        shift_millis: u8,
        max_packets: usize,
        max_bytes: usize,
        stats: ArcLimitedRecvStats,
    ) -> Self {
        LimitedRecvState {
            delay: Box::pin(tokio::time::sleep(Duration::from_millis(0))),
            started,
            limit: DatagramLimitInfo::new(shift_millis, max_packets, max_bytes),
            stats,
        }
    }
}

pub struct LimitedUdpSocket {
    inner: Box<dyn AsyncUdpSocket>,
    send_state: UnsafeCell<LimitedSendState>,
    recv_state: UnsafeCell<LimitedRecvState>,
}

impl LimitedUdpSocket {
    fn new<ST>(
        inner: Box<dyn AsyncUdpSocket>,
        shift_millis: u8,
        max_send_packets: usize,
        max_send_bytes: usize,
        max_recv_packets: usize,
        max_recv_bytes: usize,
        stats: Arc<ST>,
    ) -> Self
    where
        ST: LimitedSendStats + LimitedRecvStats + Send + Sync + 'static,
    {
        let started = Instant::now();
        let send_state = LimitedSendState::new(
            started,
            shift_millis,
            max_send_packets,
            max_send_bytes,
            stats.clone() as _,
        );
        let recv_state = LimitedRecvState::new(
            started,
            shift_millis,
            max_recv_packets,
            max_recv_bytes,
            stats as _,
        );
        LimitedUdpSocket {
            inner,
            send_state: UnsafeCell::new(send_state),
            recv_state: UnsafeCell::new(recv_state),
        }
    }
}

impl fmt::Debug for LimitedUdpSocket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl AsyncUdpSocket for LimitedUdpSocket {
    fn poll_send(
        &self,
        state: &udp::UdpState,
        cx: &mut Context,
        transmits: &[udp::Transmit],
    ) -> Poll<io::Result<usize>> {
        let len = transmits.iter().map(|t| t.contents.len()).sum();
        let l = unsafe { &mut *self.send_state.get() };
        if l.limit.is_set() {
            let dur_millis = l.started.elapsed().as_millis() as u64;
            match l.limit.check_packet(dur_millis, len) {
                DatagramLimitResult::Advance => {
                    let nw = ready!(self.inner.poll_send(state, cx, transmits))?;
                    l.limit.set_advance(1, nw);
                    l.stats.add_send_packet();
                    l.stats.add_send_bytes(nw);
                    Poll::Ready(Ok(nw))
                }
                DatagramLimitResult::DelayFor(ms) => {
                    l.delay
                        .as_mut()
                        .reset(l.started + Duration::from_millis(dur_millis + ms));
                    l.delay.poll_unpin(cx).map(|_| Ok(0))
                }
            }
        } else {
            let nw = ready!(self.inner.poll_send(state, cx, transmits))?;
            l.stats.add_send_packet();
            l.stats.add_send_bytes(nw);
            Poll::Ready(Ok(nw))
        }
    }

    fn poll_recv(
        &self,
        cx: &mut Context,
        bufs: &mut [IoSliceMut<'_>],
        meta: &mut [udp::RecvMeta],
    ) -> Poll<io::Result<usize>> {
        let len = bufs.iter().map(|v| v.len()).sum();
        let l = unsafe { &mut *self.recv_state.get() };
        if l.limit.is_set() {
            let dur_millis = l.started.elapsed().as_millis() as u64;
            match l.limit.check_packet(dur_millis, len) {
                DatagramLimitResult::Advance => {
                    let nr = ready!(self.inner.poll_recv(cx, bufs, meta))?;
                    l.limit.set_advance(1, nr);
                    l.stats.add_recv_packet();
                    l.stats.add_recv_bytes(nr);
                    Poll::Ready(Ok(nr))
                }
                DatagramLimitResult::DelayFor(ms) => {
                    l.delay
                        .as_mut()
                        .reset(l.started + Duration::from_millis(dur_millis + ms));
                    l.delay.poll_unpin(cx).map(|_| Ok(0))
                }
            }
        } else {
            let nr = ready!(self.inner.poll_recv(cx, bufs, meta))?;
            l.stats.add_recv_packet();
            l.stats.add_recv_bytes(nr);
            Poll::Ready(Ok(nr))
        }
    }

    fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.inner.local_addr()
    }

    fn may_fragment(&self) -> bool {
        self.inner.may_fragment()
    }
}
