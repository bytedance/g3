/*
 * Copyright 2024 ByteDance and/or its affiliates.
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
use std::sync::{Arc, Mutex};
use std::task::{ready, Context, Poll};
use std::time::Duration;

use futures_util::FutureExt;
use quinn::udp;
use quinn::{AsyncTimer, AsyncUdpSocket, Runtime, UdpPoller};
use tokio::time::{Instant, Sleep};

use crate::limit::{DatagramLimitInfo, DatagramLimitResult};
use crate::{ArcLimitedRecvStats, ArcLimitedSendStats, LimitedRecvStats, LimitedSendStats};

struct LimitConf {
    shift_millis: u8,
    max_send_packets: usize,
    max_send_bytes: usize,
    max_recv_packets: usize,
    max_recv_bytes: usize,
}

pub struct LimitedTokioRuntime<R, ST> {
    inner: R,
    limit: Option<LimitConf>,
    stats: Arc<ST>,
}

impl<R, ST> LimitedTokioRuntime<R, ST> {
    pub fn new(
        inner: R,
        shift_millis: u8,
        max_send_packets: usize,
        max_send_bytes: usize,
        max_recv_packets: usize,
        max_recv_bytes: usize,
        stats: Arc<ST>,
    ) -> Self {
        let limit = LimitConf {
            shift_millis,
            max_send_packets,
            max_send_bytes,
            max_recv_packets,
            max_recv_bytes,
        };
        LimitedTokioRuntime {
            inner,
            limit: Some(limit),
            stats,
        }
    }

    pub fn new_unlimited(inner: R, stats: Arc<ST>) -> Self {
        LimitedTokioRuntime {
            inner,
            limit: None,
            stats,
        }
    }
}

impl<R: Runtime, ST> fmt::Debug for LimitedTokioRuntime<R, ST> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<R: Runtime, ST> Runtime for LimitedTokioRuntime<R, ST>
where
    ST: LimitedSendStats + LimitedRecvStats + Send + Sync + 'static,
{
    fn new_timer(&self, t: std::time::Instant) -> Pin<Box<dyn AsyncTimer>> {
        self.inner.new_timer(t)
    }

    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>) {
        self.inner.spawn(future);
    }

    fn wrap_udp_socket(&self, sock: std::net::UdpSocket) -> io::Result<Arc<dyn AsyncUdpSocket>> {
        let inner = self.inner.wrap_udp_socket(sock)?;
        if let Some(limit) = &self.limit {
            Ok(Arc::new(LimitedUdpSocket::new(
                inner,
                limit.shift_millis,
                limit.max_send_packets,
                limit.max_send_bytes,
                limit.max_recv_packets,
                limit.max_recv_bytes,
                self.stats.clone(),
            )))
        } else {
            Ok(Arc::new(LimitedUdpSocket::new_unlimited(
                inner,
                self.stats.clone(),
            )))
        }
    }
}

struct LimitedSendLimitState {
    delay: Pin<Box<Sleep>>,
    poll_delay: bool,
    limit: DatagramLimitInfo,
}

struct LimitedSendState {
    started: Instant,
    limit: Option<Arc<Mutex<LimitedSendLimitState>>>,
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
        let limit = LimitedSendLimitState {
            delay: Box::pin(tokio::time::sleep(Duration::from_millis(0))),
            poll_delay: false,
            limit: DatagramLimitInfo::new(shift_millis, max_packets, max_bytes),
        };
        LimitedSendState {
            started,
            limit: Some(Arc::new(Mutex::new(limit))),
            stats,
        }
    }

    fn new_unlimited(started: Instant, stats: ArcLimitedSendStats) -> Self {
        LimitedSendState {
            started,
            limit: None,
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

    fn new_unlimited(started: Instant, stats: ArcLimitedRecvStats) -> Self {
        LimitedRecvState {
            delay: Box::pin(tokio::time::sleep(Duration::from_millis(0))),
            started,
            limit: DatagramLimitInfo::default(),
            stats,
        }
    }
}

struct LimitedUdpPoller {
    inner: Pin<Box<dyn UdpPoller>>,
    limit: Option<Arc<Mutex<LimitedSendLimitState>>>,
}

impl fmt::Debug for LimitedUdpPoller {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl UdpPoller for LimitedUdpPoller {
    fn poll_writable(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        if let Some(l) = &self.limit {
            let mut l = l.lock().unwrap();
            if l.poll_delay {
                ready!(Future::poll(l.delay.as_mut(), cx));
                l.poll_delay = false;
                return Poll::Ready(Ok(()));
            }
        }
        self.inner.as_mut().poll_writable(cx)
    }
}

pub struct LimitedUdpSocket {
    inner: Arc<dyn AsyncUdpSocket>,
    send_state: LimitedSendState,
    recv_state: UnsafeCell<LimitedRecvState>,
}

unsafe impl Sync for LimitedUdpSocket {}

impl LimitedUdpSocket {
    fn new<ST>(
        inner: Arc<dyn AsyncUdpSocket>,
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
            send_state,
            recv_state: UnsafeCell::new(recv_state),
        }
    }

    fn new_unlimited<ST>(inner: Arc<dyn AsyncUdpSocket>, stats: Arc<ST>) -> Self
    where
        ST: LimitedSendStats + LimitedRecvStats + Send + Sync + 'static,
    {
        let started = Instant::now();
        let send_state = LimitedSendState::new_unlimited(started, stats.clone() as _);
        let recv_state = LimitedRecvState::new_unlimited(started, stats as _);
        LimitedUdpSocket {
            inner,
            send_state,
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
    fn create_io_poller(self: Arc<Self>) -> Pin<Box<dyn UdpPoller>> {
        Box::pin(LimitedUdpPoller {
            inner: self.inner.clone().create_io_poller(),
            limit: self.send_state.limit.clone(),
        })
    }

    fn try_send(&self, transmit: &udp::Transmit) -> io::Result<()> {
        let len = transmit.contents.len();
        if let Some(l) = &self.send_state.limit {
            let dur_millis = self.send_state.started.elapsed().as_millis() as u64;
            let mut l = l.lock().unwrap();
            match l.limit.check_packet(dur_millis, len) {
                DatagramLimitResult::Advance(_) => {
                    self.inner.try_send(transmit)?;
                    l.limit.set_advance(1, len);
                    self.send_state.stats.add_send_packet();
                    self.send_state.stats.add_send_bytes(len);
                    Ok(())
                }
                DatagramLimitResult::DelayFor(ms) => {
                    l.delay
                        .as_mut()
                        .reset(self.send_state.started + Duration::from_millis(dur_millis + ms));
                    l.poll_delay = true;
                    Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "delayed by rate limiter",
                    ))
                }
            }
        } else {
            self.inner.try_send(transmit)?;
            self.send_state.stats.add_send_packet();
            self.send_state.stats.add_send_bytes(len);
            Ok(())
        }
    }

    fn poll_recv(
        &self,
        cx: &mut Context,
        bufs: &mut [IoSliceMut<'_>],
        meta: &mut [udp::RecvMeta],
    ) -> Poll<io::Result<usize>> {
        let l = unsafe { &mut *self.recv_state.get() };
        if l.limit.is_set() {
            let dur_millis = l.started.elapsed().as_millis() as u64;
            match l.limit.check_packets(dur_millis, bufs) {
                DatagramLimitResult::Advance(n) => {
                    let nr = ready!(self.inner.poll_recv(cx, &mut bufs[0..n], &mut meta[0..n]))?;
                    let len = bufs.iter().take(nr).map(|v| v.len()).sum();
                    l.limit.set_advance(nr, len);
                    l.stats.add_recv_packets(nr);
                    l.stats.add_recv_bytes(len);
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
            let len = bufs.iter().take(nr).map(|v| v.len()).sum();
            l.stats.add_recv_packets(nr);
            l.stats.add_recv_bytes(len);
            Poll::Ready(Ok(nr))
        }
    }

    fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.inner.local_addr()
    }

    fn max_transmit_segments(&self) -> usize {
        self.inner.max_transmit_segments()
    }

    fn max_receive_segments(&self) -> usize {
        self.inner.max_receive_segments()
    }

    fn may_fragment(&self) -> bool {
        self.inner.may_fragment()
    }
}
