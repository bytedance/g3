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
    buf_reserved: usize,
    buf_data_off: usize,
    buf_data_end: usize,
    to: UpstreamAddr,
}

impl UdpRelayPacket {
    fn new(reserved_size: usize, packet_size: usize) -> Self {
        let buf_size = packet_size + reserved_size;
        UdpRelayPacket {
            buf: vec![0; buf_size].into_boxed_slice(),
            buf_reserved: reserved_size,
            buf_data_off: 0,
            buf_data_end: 0,
            to: UpstreamAddr::empty(),
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

pub struct UdpRelayClientToRemote<C: ?Sized, R: ?Sized> {
    client: Box<C>,
    remote: Box<R>,
    config: LimitedUdpRelayConfig,
    packet: UdpRelayPacket,
    total: u64,
    active: bool,
    to_send: bool,
}

impl<C, R> UdpRelayClientToRemote<C, R>
where
    C: UdpRelayClientRecv + ?Sized,
    R: UdpRelayRemoteSend + ?Sized,
{
    pub fn new(client: Box<C>, remote: Box<R>, config: LimitedUdpRelayConfig) -> Self {
        let buf_reserved = remote.buf_reserve_length().max(client.buf_reserve_length());
        let packet = UdpRelayPacket::new(buf_reserved, config.packet_size);
        UdpRelayClientToRemote {
            client,
            remote,
            config,
            packet,
            total: 0,
            active: false,
            to_send: false,
        }
    }

    pub fn is_idle(&self) -> bool {
        !self.active
    }

    pub fn reset_active(&mut self) {
        self.active = false;
    }
}

impl<C, R> Future for UdpRelayClientToRemote<C, R>
where
    C: UdpRelayClientRecv + Unpin + ?Sized,
    R: UdpRelayRemoteSend + Unpin + ?Sized,
{
    type Output = Result<u64, UdpRelayError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut relay_this_round = 0usize;
        loop {
            let me = &mut *self;
            if !me.to_send {
                let reserved = me.packet.buf_reserved;
                let (off, nr, to) =
                    ready!(Pin::new(&mut *me.client)
                        .poll_recv_packet(cx, &mut me.packet.buf[reserved..]))?;
                if nr == 0 {
                    break;
                }
                me.packet.buf_data_off = reserved + off;
                me.packet.buf_data_end = reserved + nr;
                me.packet.to = to;
                me.to_send = true;
                me.active = true;
            }

            if me.to_send {
                let nw = ready!(Pin::new(&mut *me.remote).poll_send_packet(
                    cx,
                    &mut me.packet.buf,
                    me.packet.buf_data_off,
                    me.packet.buf_data_end,
                    &me.packet.to
                ))
                .map_err(|e| UdpRelayError::RemoteError(Some(me.packet.to.clone()), e))?;
                relay_this_round += nw;
                me.total += nw as u64;
                me.to_send = false;
                me.active = true;
            }

            if relay_this_round >= self.config.yield_size {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        }
        Poll::Ready(Ok(self.total))
    }
}

pub struct UdpRelayRemoteToClient<C: ?Sized, R: ?Sized> {
    client: Box<C>,
    remote: Box<R>,
    config: LimitedUdpRelayConfig,
    packet: UdpRelayPacket,
    total: u64,
    active: bool,
    to_send: bool,
}

impl<C, R> UdpRelayRemoteToClient<C, R>
where
    C: UdpRelayClientSend + ?Sized,
    R: UdpRelayRemoteRecv + ?Sized,
{
    pub fn new(client: Box<C>, remote: Box<R>, config: LimitedUdpRelayConfig) -> Self {
        let buf_reserved = client.buf_reserve_length().max(remote.buf_reserve_length());
        let packet = UdpRelayPacket::new(buf_reserved, config.packet_size);
        UdpRelayRemoteToClient {
            client,
            remote,
            config,
            packet,
            total: 0,
            active: false,
            to_send: false,
        }
    }

    pub fn is_idle(&self) -> bool {
        !self.active
    }

    pub fn reset_active(&mut self) {
        self.active = false;
    }
}

impl<C, R> Future for UdpRelayRemoteToClient<C, R>
where
    C: UdpRelayClientSend + Unpin + ?Sized,
    R: UdpRelayRemoteRecv + Unpin + ?Sized,
{
    type Output = Result<u64, UdpRelayError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut relay_this_round = 0usize;
        loop {
            let me = &mut *self;
            if !me.to_send {
                let reserved = me.packet.buf_reserved;
                let (off, nr, to) =
                    ready!(Pin::new(&mut *me.remote)
                        .poll_recv_packet(cx, &mut me.packet.buf[reserved..]))
                    .map_err(|e| UdpRelayError::RemoteError(None, e))?;
                if nr == 0 {
                    break;
                }
                me.packet.buf_data_off = reserved + off;
                me.packet.buf_data_end = reserved + nr;
                me.packet.to = to;
                me.to_send = true;
                me.active = true;
            }

            if me.to_send {
                let nw = ready!(Pin::new(&mut *me.client).poll_send_packet(
                    cx,
                    &mut me.packet.buf,
                    me.packet.buf_data_off,
                    me.packet.buf_data_end,
                    &me.packet.to
                ))?;
                relay_this_round += nw;
                me.total += nw as u64;
                me.to_send = false;
                me.active = true;
            }

            if relay_this_round > self.config.yield_size {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        }
        Poll::Ready(Ok(self.total))
    }
}
