/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::cell::UnsafeCell;
use std::fmt;
use std::io::{self, IoSliceMut};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, ready};
use std::time::Duration;

use futures_util::FutureExt;
use quinn::udp;
use quinn::{AsyncTimer, AsyncUdpSocket, Runtime, UdpPoller};
use smallvec::SmallVec;
use tokio::time::{Instant, Sleep};

use crate::limit::{DatagramLimitAction, DatagramLimiter};
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
    pub fn new(inner: R, stats: Arc<ST>) -> Self {
        LimitedTokioRuntime {
            inner,
            limit: None,
            stats,
        }
    }

    pub fn local_limited(
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
            Ok(Arc::new(LimitedUdpSocket::local_limited(
                inner,
                limit.shift_millis,
                limit.max_send_packets,
                limit.max_send_bytes,
                limit.max_recv_packets,
                limit.max_recv_bytes,
                self.stats.clone(),
            )))
        } else {
            Ok(Arc::new(LimitedUdpSocket::new(inner, self.stats.clone())))
        }
    }
}

struct LimitedSendLimitState {
    delay: Pin<Box<Sleep>>,
    poll_delay: bool,
    limit: DatagramLimiter,
}

struct LimitedSendState {
    started: Instant,
    limit: Option<Arc<Mutex<LimitedSendLimitState>>>,
    stats: ArcLimitedSendStats,
}

impl LimitedSendState {
    fn new(started: Instant, stats: ArcLimitedSendStats) -> Self {
        LimitedSendState {
            started,
            limit: None,
            stats,
        }
    }

    fn local_limited(
        started: Instant,
        shift_millis: u8,
        max_packets: usize,
        max_bytes: usize,
        stats: ArcLimitedSendStats,
    ) -> Self {
        let limit = LimitedSendLimitState {
            delay: Box::pin(tokio::time::sleep(Duration::from_millis(0))),
            poll_delay: false,
            limit: DatagramLimiter::with_local(shift_millis, max_packets, max_bytes),
        };
        LimitedSendState {
            started,
            limit: Some(Arc::new(Mutex::new(limit))),
            stats,
        }
    }
}

struct LimitedRecvState {
    delay: Pin<Box<Sleep>>,
    started: Instant,
    limit: DatagramLimiter,
    stats: ArcLimitedRecvStats,
}

impl LimitedRecvState {
    fn new(started: Instant, stats: ArcLimitedRecvStats) -> Self {
        LimitedRecvState {
            delay: Box::pin(tokio::time::sleep(Duration::from_millis(0))),
            started,
            limit: DatagramLimiter::default(),
            stats,
        }
    }

    fn local_limited(
        started: Instant,
        shift_millis: u8,
        max_packets: usize,
        max_bytes: usize,
        stats: ArcLimitedRecvStats,
    ) -> Self {
        LimitedRecvState {
            delay: Box::pin(tokio::time::sleep(Duration::from_millis(0))),
            started,
            limit: DatagramLimiter::with_local(shift_millis, max_packets, max_bytes),
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
    fn new<ST>(inner: Arc<dyn AsyncUdpSocket>, stats: Arc<ST>) -> Self
    where
        ST: LimitedSendStats + LimitedRecvStats + Send + Sync + 'static,
    {
        let started = Instant::now();
        let send_state = LimitedSendState::new(started, stats.clone());
        let recv_state = LimitedRecvState::new(started, stats);
        LimitedUdpSocket {
            inner,
            send_state,
            recv_state: UnsafeCell::new(recv_state),
        }
    }

    fn local_limited<ST>(
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
        let send_state = LimitedSendState::local_limited(
            started,
            shift_millis,
            max_send_packets,
            max_send_bytes,
            stats.clone(),
        );
        let recv_state = LimitedRecvState::local_limited(
            started,
            shift_millis,
            max_recv_packets,
            max_recv_bytes,
            stats,
        );
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
                DatagramLimitAction::Advance(_) => match self.inner.try_send(transmit) {
                    Ok(_) => {
                        self.inner.try_send(transmit)?;
                        l.limit.set_advance(1, len);
                        self.send_state.stats.add_send_packet();
                        self.send_state.stats.add_send_bytes(len);
                        Ok(())
                    }
                    Err(e) => {
                        l.limit.release_global();
                        Err(e)
                    }
                },
                DatagramLimitAction::DelayUntil(t) => {
                    l.delay.as_mut().reset(t);
                    l.poll_delay = true;
                    Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "delayed by rate limiter",
                    ))
                }
                DatagramLimitAction::DelayFor(ms) => {
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
            let mut total_size_v = SmallVec::<[usize; 32]>::with_capacity(meta.len());
            let mut total_size = 0;
            for b in bufs.iter() {
                total_size += b.len();
                total_size_v.push(total_size);
            }
            match l.limit.check_packets(dur_millis, total_size_v.as_ref()) {
                DatagramLimitAction::Advance(n) => {
                    match self.inner.poll_recv(cx, &mut bufs[0..n], &mut meta[0..n]) {
                        Poll::Ready(Ok(nr)) => {
                            let len = meta.iter().take(nr).map(|m| m.len).sum();
                            l.limit.set_advance(nr, len);
                            l.stats.add_recv_packets(nr);
                            l.stats.add_recv_bytes(len);
                            Poll::Ready(Ok(nr))
                        }
                        Poll::Ready(Err(e)) => {
                            l.limit.release_global();
                            Poll::Ready(Err(e))
                        }
                        Poll::Pending => {
                            l.limit.release_global();
                            Poll::Pending
                        }
                    }
                }
                DatagramLimitAction::DelayUntil(t) => {
                    l.delay.as_mut().reset(t);
                    match l.delay.poll_unpin(cx) {
                        Poll::Ready(_) => {
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        }
                        Poll::Pending => Poll::Pending,
                    }
                }
                DatagramLimitAction::DelayFor(ms) => {
                    l.delay
                        .as_mut()
                        .reset(l.started + Duration::from_millis(dur_millis + ms));
                    match l.delay.poll_unpin(cx) {
                        Poll::Ready(_) => {
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        }
                        Poll::Pending => Poll::Pending,
                    }
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
