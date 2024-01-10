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

use std::future::Future;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use thiserror::Error;

use g3_types::net::UpstreamAddr;

use super::LimitedUdpRelayConfig;

mod client;
mod remote;

pub use client::{UdpRelayClientError, UdpRelayClientRecv, UdpRelayClientSend};
pub use remote::{UdpRelayRemoteError, UdpRelayRemoteRecv, UdpRelayRemoteSend};

#[derive(Clone)]
pub struct UdpRelayPacket {
    buf: Box<[u8]>,
    buf_data_off: usize,
    buf_data_end: usize,
    ups: UpstreamAddr,
}

impl UdpRelayPacket {
    fn new(reserved_size: usize, packet_size: usize) -> Self {
        let buf_size = packet_size + reserved_size;
        UdpRelayPacket {
            buf: vec![0; buf_size].into_boxed_slice(),
            buf_data_off: 0,
            buf_data_end: 0,
            ups: UpstreamAddr::empty(),
        }
    }

    #[inline]
    pub fn buf_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }

    #[inline]
    pub fn buf(&self) -> &[u8] {
        &self.buf
    }

    #[inline]
    pub fn set_offset(&mut self, off: usize) {
        self.buf_data_off = off;
    }

    #[inline]
    pub fn set_length(&mut self, len: usize) {
        self.buf_data_end = len;
    }

    #[inline]
    pub fn set_upstream(&mut self, ups: UpstreamAddr) {
        self.ups = ups;
    }

    #[inline]
    pub fn upstream(&self) -> &UpstreamAddr {
        &self.ups
    }

    #[inline]
    pub fn payload(&self) -> &[u8] {
        &self.buf[self.buf_data_off..self.buf_data_end]
    }
}

#[derive(Error, Debug)]
pub enum UdpRelayError {
    #[error("client: {0}")]
    ClientError(#[from] UdpRelayClientError),
    #[error("remote: {1}")]
    RemoteError(Option<UpstreamAddr>, UdpRelayRemoteError),
}

trait UdpRelayRecv {
    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        packet: &mut UdpRelayPacket,
    ) -> Poll<Result<usize, UdpRelayError>>;

    fn poll_recv_packets(
        &mut self,
        cx: &mut Context<'_>,
        packets: &mut [UdpRelayPacket],
    ) -> Poll<Result<usize, UdpRelayError>> {
        let mut count = 0;
        for packet in packets.iter_mut() {
            match self.poll_recv_packet(cx, packet) {
                Poll::Pending => {
                    return if count > 0 {
                        Poll::Ready(Ok(count))
                    } else {
                        Poll::Pending
                    };
                }
                Poll::Ready(Ok(_)) => count += 1,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            }
        }
        Poll::Ready(Ok(count))
    }
}

struct ClientRecv<'a, T: UdpRelayClientRecv + ?Sized>(&'a mut T);

impl<'a, T: UdpRelayClientRecv + ?Sized> UdpRelayRecv for ClientRecv<'a, T> {
    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        packet: &mut UdpRelayPacket,
    ) -> Poll<Result<usize, UdpRelayError>> {
        let (off, nr, ups) = ready!(self
            .0
            .poll_recv_packet(cx, &mut packet.buf)
            .map_err(UdpRelayError::ClientError))?;
        packet.buf_data_off = off;
        packet.buf_data_end = nr;
        packet.ups = ups;
        Poll::Ready(Ok(nr))
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    fn poll_recv_packets(
        &mut self,
        cx: &mut Context<'_>,
        packets: &mut [UdpRelayPacket],
    ) -> Poll<Result<usize, UdpRelayError>> {
        self.0
            .poll_recv_packets(cx, packets)
            .map_err(UdpRelayError::ClientError)
    }
}

struct RemoteRecv<'a, T: UdpRelayRemoteRecv + ?Sized>(&'a mut T);

impl<'a, T: UdpRelayRemoteRecv + ?Sized> UdpRelayRecv for RemoteRecv<'a, T> {
    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        packet: &mut UdpRelayPacket,
    ) -> Poll<Result<usize, UdpRelayError>> {
        let (off, nr, ups) = ready!(self
            .0
            .poll_recv_packet(cx, &mut packet.buf)
            .map_err(|e| UdpRelayError::RemoteError(None, e)))?;
        packet.buf_data_off = off;
        packet.buf_data_end = nr;
        packet.ups = ups;
        Poll::Ready(Ok(nr))
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    fn poll_recv_packets(
        &mut self,
        cx: &mut Context<'_>,
        packets: &mut [UdpRelayPacket],
    ) -> Poll<Result<usize, UdpRelayError>> {
        self.0
            .poll_recv_packets(cx, packets)
            .map_err(|e| UdpRelayError::RemoteError(None, e))
    }
}

trait UdpRelaySend {
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        packet: &UdpRelayPacket,
    ) -> Poll<Result<usize, UdpRelayError>>;

    fn poll_send_packets(
        &mut self,
        cx: &mut Context<'_>,
        packets: &[UdpRelayPacket],
    ) -> Poll<Result<usize, UdpRelayError>> {
        let mut count = 0;
        for packet in packets {
            match self.poll_send_packet(cx, packet) {
                Poll::Pending => {
                    return if count > 0 {
                        Poll::Ready(Ok(count))
                    } else {
                        Poll::Pending
                    };
                }
                Poll::Ready(Ok(_)) => count += 1,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            }
        }
        Poll::Ready(Ok(count))
    }
}

struct ClientSend<'a, T: UdpRelayClientSend + ?Sized>(&'a mut T);

impl<'a, T: UdpRelayClientSend + ?Sized> UdpRelaySend for ClientSend<'a, T> {
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        packet: &UdpRelayPacket,
    ) -> Poll<Result<usize, UdpRelayError>> {
        self.0
            .poll_send_packet(cx, packet.payload(), &packet.ups)
            .map_err(UdpRelayError::ClientError)
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    fn poll_send_packets(
        &mut self,
        cx: &mut Context<'_>,
        packets: &[UdpRelayPacket],
    ) -> Poll<Result<usize, UdpRelayError>> {
        self.0
            .poll_send_packets(cx, packets)
            .map_err(UdpRelayError::ClientError)
    }
}

struct RemoteSend<'a, T: UdpRelayRemoteSend + ?Sized>(&'a mut T);

impl<'a, T: UdpRelayRemoteSend + ?Sized> UdpRelaySend for RemoteSend<'a, T> {
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        packet: &UdpRelayPacket,
    ) -> Poll<Result<usize, UdpRelayError>> {
        self.0
            .poll_send_packet(cx, packet.payload(), &packet.ups)
            .map_err(|e| UdpRelayError::RemoteError(Some(packet.ups.clone()), e))
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    fn poll_send_packets(
        &mut self,
        cx: &mut Context<'_>,
        packets: &[UdpRelayPacket],
    ) -> Poll<Result<usize, UdpRelayError>> {
        self.0
            .poll_send_packets(cx, packets)
            .map_err(|e| UdpRelayError::RemoteError(None, e))
    }
}

struct UdpRelayBuffer {
    config: LimitedUdpRelayConfig,
    packets: Vec<UdpRelayPacket>,
    send_start: usize,
    send_end: usize,
    recv_done: bool,
    total: u64,
    active: bool,
}

impl UdpRelayBuffer {
    fn new(max_hdr_size: usize, config: LimitedUdpRelayConfig) -> Self {
        let packets =
            vec![UdpRelayPacket::new(max_hdr_size, config.packet_size); config.batch_size];
        UdpRelayBuffer {
            config,
            packets,
            send_start: 0,
            send_end: 0,
            recv_done: false,
            total: 0,
            active: false,
        }
    }

    fn poll_batch_relay<R, S>(
        &mut self,
        cx: &mut Context<'_>,
        mut receiver: R,
        mut sender: S,
    ) -> Poll<Result<u64, UdpRelayError>>
    where
        R: UdpRelayRecv,
        S: UdpRelaySend,
    {
        let mut copy_this_round = 0usize;
        loop {
            if !self.recv_done && self.send_end < self.packets.len() {
                match receiver.poll_recv_packets(cx, &mut self.packets[self.send_end..]) {
                    Poll::Ready(Ok(count)) => {
                        if count == 0 {
                            self.recv_done = true;
                        }
                        self.send_end += count;
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => {
                        if self.send_start >= self.send_end {
                            return Poll::Pending;
                        }
                    }
                }
            }

            while self.send_end > self.send_start {
                let packets = &self.packets[self.send_start..self.send_end];
                let count = ready!(sender.poll_send_packets(cx, packets))?;
                copy_this_round += packets
                    .iter()
                    .take(count)
                    .map(|p| p.buf_data_end - p.buf_data_off)
                    .sum::<usize>();
                self.send_start += count;
            }
            self.send_start = 0;
            self.send_end = 0;

            if copy_this_round >= self.config.yield_size {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }

            if self.recv_done {
                return Poll::Ready(Ok(self.total));
            }
        }
    }

    fn is_idle(&self) -> bool {
        !self.active
    }

    fn reset_active(&mut self) {
        self.active = false;
    }
}

pub struct UdpRelayClientToRemote<'a, C: ?Sized, R: ?Sized> {
    client: &'a mut C,
    remote: &'a mut R,
    buffer: UdpRelayBuffer,
}

impl<'a, C, R> UdpRelayClientToRemote<'a, C, R>
where
    C: UdpRelayClientRecv + ?Sized,
    R: UdpRelayRemoteSend + ?Sized,
{
    pub fn new(client: &'a mut C, remote: &'a mut R, config: LimitedUdpRelayConfig) -> Self {
        let buffer = UdpRelayBuffer::new(client.max_hdr_len(), config);
        UdpRelayClientToRemote {
            client,
            remote,
            buffer,
        }
    }

    #[inline]
    pub fn is_idle(&self) -> bool {
        self.buffer.is_idle()
    }

    #[inline]
    pub fn reset_active(&mut self) {
        self.buffer.reset_active()
    }
}

impl<'a, C, R> Future for UdpRelayClientToRemote<'a, C, R>
where
    C: UdpRelayClientRecv + Unpin + ?Sized,
    R: UdpRelayRemoteSend + Unpin + ?Sized,
{
    type Output = Result<u64, UdpRelayError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;
        me.buffer
            .poll_batch_relay(cx, ClientRecv(me.client), RemoteSend(me.remote))
    }
}

pub struct UdpRelayRemoteToClient<'a, C: ?Sized, R: ?Sized> {
    client: &'a mut C,
    remote: &'a mut R,
    buffer: UdpRelayBuffer,
}

impl<'a, C, R> UdpRelayRemoteToClient<'a, C, R>
where
    C: UdpRelayClientSend + ?Sized,
    R: UdpRelayRemoteRecv + ?Sized,
{
    pub fn new(client: &'a mut C, remote: &'a mut R, config: LimitedUdpRelayConfig) -> Self {
        let buffer = UdpRelayBuffer::new(remote.max_hdr_len(), config);
        UdpRelayRemoteToClient {
            client,
            remote,
            buffer,
        }
    }

    #[inline]
    pub fn is_idle(&self) -> bool {
        self.buffer.is_idle()
    }

    #[inline]
    pub fn reset_active(&mut self) {
        self.buffer.reset_active()
    }
}

impl<'a, C, R> Future for UdpRelayRemoteToClient<'a, C, R>
where
    C: UdpRelayClientSend + Unpin + ?Sized,
    R: UdpRelayRemoteRecv + Unpin + ?Sized,
{
    type Output = Result<u64, UdpRelayError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;
        me.buffer
            .poll_batch_relay(cx, RemoteRecv(me.remote), ClientSend(me.client))
    }
}
