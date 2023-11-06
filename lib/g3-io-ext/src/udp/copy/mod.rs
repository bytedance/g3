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

struct UdpCopyPacket {
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
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize), UdpCopyError>>;
}

struct ClientRecv<'a, T: UdpCopyClientRecv + ?Sized>(&'a mut T);

impl<'a, T: UdpCopyClientRecv + ?Sized> UdpCopyRecv for ClientRecv<'a, T> {
    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize), UdpCopyError>> {
        self.0
            .poll_recv_packet(cx, buf)
            .map_err(UdpCopyError::ClientError)
    }
}

struct RemoteRecv<'a, T: UdpCopyRemoteRecv + ?Sized>(&'a mut T);

impl<'a, T: UdpCopyRemoteRecv + ?Sized> UdpCopyRecv for RemoteRecv<'a, T> {
    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize), UdpCopyError>> {
        self.0
            .poll_recv_packet(cx, buf)
            .map_err(UdpCopyError::RemoteError)
    }
}

trait UdpCopySend {
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, UdpCopyError>>;
}

struct ClientSend<'a, T: UdpCopyClientSend + ?Sized>(&'a mut T);

impl<'a, T: UdpCopyClientSend + ?Sized> UdpCopySend for ClientSend<'a, T> {
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, UdpCopyError>> {
        self.0
            .poll_send_packet(cx, buf)
            .map_err(UdpCopyError::ClientError)
    }
}

struct RemoteSend<'a, T: UdpCopyRemoteSend + ?Sized>(&'a mut T);

impl<'a, T: UdpCopyRemoteSend + ?Sized> UdpCopySend for RemoteSend<'a, T> {
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, UdpCopyError>> {
        self.0
            .poll_send_packet(cx, buf)
            .map_err(UdpCopyError::RemoteError)
    }
}

struct UdpCopyBuffer {
    config: LimitedUdpRelayConfig,
    packet: UdpCopyPacket,
    total: u64,
    active: bool,
    to_send: bool,
}

impl UdpCopyBuffer {
    fn new(max_hdr_size: usize, config: LimitedUdpRelayConfig) -> Self {
        let packet = UdpCopyPacket::new(max_hdr_size, config.packet_size);
        UdpCopyBuffer {
            config,
            packet,
            total: 0,
            active: false,
            to_send: false,
        }
    }

    fn poll_copy<R, S>(
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
            if !self.to_send {
                let (off, nr) = ready!(receiver.poll_recv_packet(cx, &mut self.packet.buf))?;
                if nr == 0 {
                    break;
                }
                self.packet.buf_data_off = off;
                self.packet.buf_data_end = nr;
                self.to_send = true;
                self.active = true;
            }

            if self.to_send {
                let nw = ready!(sender.poll_send_packet(
                    cx,
                    &self.packet.buf[self.packet.buf_data_off..self.packet.buf_data_end],
                ))?;
                copy_this_round += nw;
                self.total += nw as u64;
                self.to_send = false;
                self.active = true;
            }

            if copy_this_round >= self.config.yield_size {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        }
        Poll::Ready(Ok(self.total))
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
            .poll_copy(cx, ClientRecv(me.client), RemoteSend(me.remote))
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
            .poll_copy(cx, RemoteRecv(&mut *me.remote), ClientSend(&mut *me.client))
    }
}
