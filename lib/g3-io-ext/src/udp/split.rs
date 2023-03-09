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

use std::error::Error;
use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::task::{ready, Context, Poll};

use tokio::io::ReadBuf;
use tokio::net::UdpSocket;

use super::{AsyncUdpRecv, AsyncUdpSend};

#[derive(Debug)]
pub struct SendHalf(Arc<UdpSocket>);

#[derive(Debug)]
pub struct RecvHalf(Arc<UdpSocket>);

pub fn split(socket: UdpSocket) -> (RecvHalf, SendHalf) {
    let shared = Arc::new(socket);
    let send = shared.clone();
    let recv = shared;
    (RecvHalf(recv), SendHalf(send))
}

#[derive(Debug)]
pub struct ReuniteError(pub SendHalf, pub RecvHalf);

impl fmt::Display for ReuniteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tried to reunite halves that are not from the same socket"
        )
    }
}

impl Error for ReuniteError {}

fn reunite(s: SendHalf, r: RecvHalf) -> Result<UdpSocket, ReuniteError> {
    if Arc::ptr_eq(&s.0, &r.0) {
        drop(r);
        // Only two instances of the `Arc` are ever created, one for the
        // receiver and one for the sender, and those `Arc`s are never exposed
        // externally. And so when we drop one here, the other one must be the
        // only remaining one.
        Ok(Arc::try_unwrap(s.0).expect("udp: try_unwrap failed in reunite"))
    } else {
        Err(ReuniteError(s, r))
    }
}

impl SendHalf {
    pub fn reunite(self, other: RecvHalf) -> Result<UdpSocket, ReuniteError> {
        reunite(self, other)
    }
}

impl AsyncUdpSend for SendHalf {
    fn poll_send_to(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        target: SocketAddr,
    ) -> Poll<io::Result<usize>> {
        self.0.poll_send_to(cx, buf, target)
    }

    fn poll_send(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.0.poll_send(cx, buf)
    }
}

impl RecvHalf {
    pub fn reunite(self, other: SendHalf) -> Result<UdpSocket, ReuniteError> {
        reunite(other, self)
    }

    pub async fn connect(&self, addr: SocketAddr) -> io::Result<()> {
        self.0.connect(addr).await
    }
}

impl AsyncUdpRecv for RecvHalf {
    fn poll_recv_from(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, SocketAddr), io::Error>> {
        let mut buf = ReadBuf::new(buf);
        let addr = ready!(self.0.poll_recv_from(cx, &mut buf))?;
        Poll::Ready(Ok((buf.filled().len(), addr)))
    }

    fn poll_recv(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, io::Error>> {
        let mut buf = ReadBuf::new(buf);
        ready!(self.0.poll_recv(cx, &mut buf))?;
        Poll::Ready(Ok(buf.filled().len()))
    }
}
