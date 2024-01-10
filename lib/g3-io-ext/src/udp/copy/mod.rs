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

use super::LimitedUdpRelayConfig;

mod client;
mod remote;

pub use client::{UdpCopyClientError, UdpCopyClientRecv, UdpCopyClientSend};
pub use remote::{UdpCopyRemoteError, UdpCopyRemoteRecv, UdpCopyRemoteSend};

#[derive(Clone)]
pub struct UdpCopyPacket {
    buf: Box<[u8]>,
    buf_data_off: usize,
    buf_data_end: usize,
}

impl UdpCopyPacket {
    fn new(reserved_size: usize, packet_size: usize) -> Self {
        let buf_size = packet_size + reserved_size;
        UdpCopyPacket {
            buf: vec![0; buf_size].into_boxed_slice(),
            buf_data_off: 0,
            buf_data_end: 0,
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
    pub fn payload(&self) -> &[u8] {
        &self.buf[self.buf_data_off..self.buf_data_end]
    }
}

#[derive(Error, Debug)]
pub enum UdpCopyError {
    #[error("client: {0}")]
    ClientError(#[from] UdpCopyClientError),
    #[error("remote: {0}")]
    RemoteError(#[from] UdpCopyRemoteError),
}

trait UdpCopyRecv {
    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        packet: &mut UdpCopyPacket,
    ) -> Poll<Result<usize, UdpCopyError>>;

    fn poll_recv_packets(
        &mut self,
        cx: &mut Context<'_>,
        packets: &mut [UdpCopyPacket],
    ) -> Poll<Result<usize, UdpCopyError>> {
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

struct ClientRecv<'a, T: UdpCopyClientRecv + ?Sized>(&'a mut T);

impl<'a, T: UdpCopyClientRecv + ?Sized> UdpCopyRecv for ClientRecv<'a, T> {
    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        packet: &mut UdpCopyPacket,
    ) -> Poll<Result<usize, UdpCopyError>> {
        let (off, nr) = ready!(self
            .0
            .poll_recv_packet(cx, &mut packet.buf)
            .map_err(UdpCopyError::ClientError))?;
        packet.buf_data_off = off;
        packet.buf_data_end = nr;
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
        packets: &mut [UdpCopyPacket],
    ) -> Poll<Result<usize, UdpCopyError>> {
        self.0
            .poll_recv_packets(cx, packets)
            .map_err(UdpCopyError::ClientError)
    }
}

struct RemoteRecv<'a, T: UdpCopyRemoteRecv + ?Sized>(&'a mut T);

impl<'a, T: UdpCopyRemoteRecv + ?Sized> UdpCopyRecv for RemoteRecv<'a, T> {
    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        packet: &mut UdpCopyPacket,
    ) -> Poll<Result<usize, UdpCopyError>> {
        let (off, nr) = ready!(self
            .0
            .poll_recv_packet(cx, &mut packet.buf)
            .map_err(UdpCopyError::RemoteError))?;
        packet.buf_data_off = off;
        packet.buf_data_end = nr;
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
        packets: &mut [UdpCopyPacket],
    ) -> Poll<Result<usize, UdpCopyError>> {
        self.0
            .poll_recv_packets(cx, packets)
            .map_err(UdpCopyError::RemoteError)
    }
}

trait UdpCopySend {
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        packet: &UdpCopyPacket,
    ) -> Poll<Result<usize, UdpCopyError>>;

    fn poll_send_packets(
        &mut self,
        cx: &mut Context<'_>,
        packets: &[UdpCopyPacket],
    ) -> Poll<Result<usize, UdpCopyError>> {
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

struct ClientSend<'a, T: UdpCopyClientSend + ?Sized>(&'a mut T);

impl<'a, T: UdpCopyClientSend + ?Sized> UdpCopySend for ClientSend<'a, T> {
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        packet: &UdpCopyPacket,
    ) -> Poll<Result<usize, UdpCopyError>> {
        self.0
            .poll_send_packet(cx, packet.payload())
            .map_err(UdpCopyError::ClientError)
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
        packets: &[UdpCopyPacket],
    ) -> Poll<Result<usize, UdpCopyError>> {
        self.0
            .poll_send_packets(cx, packets)
            .map_err(UdpCopyError::ClientError)
    }
}

struct RemoteSend<'a, T: UdpCopyRemoteSend + ?Sized>(&'a mut T);

impl<'a, T: UdpCopyRemoteSend + ?Sized> UdpCopySend for RemoteSend<'a, T> {
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        packet: &UdpCopyPacket,
    ) -> Poll<Result<usize, UdpCopyError>> {
        self.0
            .poll_send_packet(cx, packet.payload())
            .map_err(UdpCopyError::RemoteError)
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
        packets: &[UdpCopyPacket],
    ) -> Poll<Result<usize, UdpCopyError>> {
        self.0
            .poll_send_packets(cx, packets)
            .map_err(UdpCopyError::RemoteError)
    }
}

struct UdpCopyBuffer {
    config: LimitedUdpRelayConfig,
    packets: Vec<UdpCopyPacket>,
    send_start: usize,
    send_end: usize,
    recv_done: bool,
    total: u64,
    active: bool,
}

impl UdpCopyBuffer {
    fn new(max_hdr_size: usize, config: LimitedUdpRelayConfig) -> Self {
        let packets = vec![UdpCopyPacket::new(max_hdr_size, config.packet_size); config.batch_size];
        UdpCopyBuffer {
            config,
            packets,
            send_start: 0,
            send_end: 0,
            recv_done: false,
            total: 0,
            active: false,
        }
    }

    fn poll_batch_copy<R, S>(
        &mut self,
        cx: &mut Context<'_>,
        mut receiver: R,
        mut sender: S,
    ) -> Poll<Result<u64, UdpCopyError>>
    where
        R: UdpCopyRecv,
        S: UdpCopySend,
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

pub struct UdpCopyClientToRemote<'a, C: ?Sized, R: ?Sized> {
    client: &'a mut C,
    remote: &'a mut R,
    buffer: UdpCopyBuffer,
}

impl<'a, C, R> UdpCopyClientToRemote<'a, C, R>
where
    C: UdpCopyClientRecv + ?Sized,
    R: UdpCopyRemoteSend + ?Sized,
{
    pub fn new(client: &'a mut C, remote: &'a mut R, config: LimitedUdpRelayConfig) -> Self {
        let buffer = UdpCopyBuffer::new(client.max_hdr_len(), config);
        UdpCopyClientToRemote {
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

impl<'a, C, R> Future for UdpCopyClientToRemote<'a, C, R>
where
    C: UdpCopyClientRecv + Unpin + ?Sized,
    R: UdpCopyRemoteSend + Unpin + ?Sized,
{
    type Output = Result<u64, UdpCopyError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;
        me.buffer
            .poll_batch_copy(cx, ClientRecv(me.client), RemoteSend(me.remote))
    }
}

pub struct UdpCopyRemoteToClient<'a, C: ?Sized, R: ?Sized> {
    client: &'a mut C,
    remote: &'a mut R,
    buffer: UdpCopyBuffer,
}

impl<'a, C, R> UdpCopyRemoteToClient<'a, C, R>
where
    C: UdpCopyClientSend + ?Sized,
    R: UdpCopyRemoteRecv + ?Sized,
{
    pub fn new(client: &'a mut C, remote: &'a mut R, config: LimitedUdpRelayConfig) -> Self {
        let buffer = UdpCopyBuffer::new(remote.max_hdr_len(), config);
        UdpCopyRemoteToClient {
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

impl<'a, C, R> Future for UdpCopyRemoteToClient<'a, C, R>
where
    C: UdpCopyClientSend + Unpin + ?Sized,
    R: UdpCopyRemoteRecv + Unpin + ?Sized,
{
    type Output = Result<u64, UdpCopyError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;
        me.buffer
            .poll_batch_copy(cx, RemoteRecv(&mut *me.remote), ClientSend(&mut *me.client))
    }
}
