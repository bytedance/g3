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

struct UdpRelayPacket {
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
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize, UpstreamAddr), UdpRelayError>>;
}

struct ClientRecv<'a, T: UdpRelayClientRecv + ?Sized>(&'a mut T);

impl<'a, T: UdpRelayClientRecv + ?Sized> UdpRelayRecv for ClientRecv<'a, T> {
    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize, UpstreamAddr), UdpRelayError>> {
        self.0
            .poll_recv_packet(cx, buf)
            .map_err(UdpRelayError::ClientError)
    }
}

struct RemoteRecv<'a, T: UdpRelayRemoteRecv + ?Sized>(&'a mut T);

impl<'a, T: UdpRelayRemoteRecv + ?Sized> UdpRelayRecv for RemoteRecv<'a, T> {
    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize, UpstreamAddr), UdpRelayError>> {
        self.0
            .poll_recv_packet(cx, buf)
            .map_err(|e| UdpRelayError::RemoteError(None, e))
    }
}

trait UdpRelaySend {
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        ups: &UpstreamAddr,
    ) -> Poll<Result<usize, UdpRelayError>>;
}

struct ClientSend<'a, T: UdpRelayClientSend + ?Sized>(&'a mut T);

impl<'a, T: UdpRelayClientSend + ?Sized> UdpRelaySend for ClientSend<'a, T> {
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        from: &UpstreamAddr,
    ) -> Poll<Result<usize, UdpRelayError>> {
        self.0
            .poll_send_packet(cx, buf, from)
            .map_err(UdpRelayError::ClientError)
    }
}

struct RemoteSend<'a, T: UdpRelayRemoteSend + ?Sized>(&'a mut T);

impl<'a, T: UdpRelayRemoteSend + ?Sized> UdpRelaySend for RemoteSend<'a, T> {
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        to: &UpstreamAddr,
    ) -> Poll<Result<usize, UdpRelayError>> {
        self.0
            .poll_send_packet(cx, buf, to)
            .map_err(|e| UdpRelayError::RemoteError(Some(to.clone()), e))
    }
}

struct UdpRelayBuffer {
    config: LimitedUdpRelayConfig,
    packet: UdpRelayPacket,
    total: u64,
    active: bool,
    to_send: bool,
}

impl UdpRelayBuffer {
    fn new(max_hdr_size: usize, config: LimitedUdpRelayConfig) -> Self {
        let packet = UdpRelayPacket::new(max_hdr_size, config.packet_size);
        UdpRelayBuffer {
            config,
            packet,
            total: 0,
            active: false,
            to_send: false,
        }
    }

    fn poll_relay<R, S>(
        &mut self,
        cx: &mut Context<'_>,
        mut receiver: R,
        mut sender: S,
    ) -> Poll<Result<u64, UdpRelayError>>
    where
        R: UdpRelayRecv,
        S: UdpRelaySend,
    {
        let mut relay_this_round = 0usize;
        loop {
            if !self.to_send {
                let (off, nr, ups) = ready!(receiver.poll_recv_packet(cx, &mut self.packet.buf))?;
                if nr == 0 {
                    break;
                }
                self.packet.buf_data_off = off;
                self.packet.buf_data_end = nr;
                self.packet.ups = ups;
                self.to_send = true;
                self.active = true;
            }

            if self.to_send {
                let nw = ready!(sender.poll_send_packet(
                    cx,
                    &self.packet.buf[self.packet.buf_data_off..self.packet.buf_data_end],
                    &self.packet.ups
                ))?;
                relay_this_round += nw;
                self.total += nw as u64;
                self.to_send = false;
                self.active = true;
            }

            if relay_this_round >= self.config.yield_size {
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
            .poll_relay(cx, ClientRecv(me.client), RemoteSend(me.remote))
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
            .poll_relay(cx, RemoteRecv(me.remote), ClientSend(me.client))
    }
}
