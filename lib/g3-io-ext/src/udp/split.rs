/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::task::{Context, Poll, ready};
use std::{fmt, io};

use tokio::io::ReadBuf;
use tokio::net::UdpSocket;

use g3_io_sys::udp::{RecvMsgHdr, SendMsgHdr};

use super::{AsyncUdpRecv, AsyncUdpSend, UdpSocketExt};

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
        f.write_str("tried to reunite halves that are not from the same socket")
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

    fn poll_sendmsg<const C: usize>(
        &mut self,
        cx: &mut Context<'_>,
        hdr: &SendMsgHdr<'_, C>,
    ) -> Poll<io::Result<usize>> {
        self.0.poll_sendmsg(cx, hdr)
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    fn poll_batch_sendmsg<const C: usize>(
        &mut self,
        cx: &mut Context<'_>,
        msgs: &mut [SendMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>> {
        self.0.poll_batch_sendmsg(cx, msgs)
    }

    #[cfg(target_os = "macos")]
    fn poll_batch_sendmsg_x<const C: usize>(
        &mut self,
        cx: &mut Context<'_>,
        msgs: &mut [SendMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>> {
        self.0.poll_batch_sendmsg_x(cx, msgs)
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
    ) -> Poll<io::Result<(usize, SocketAddr)>> {
        let mut buf = ReadBuf::new(buf);
        let addr = ready!(self.0.poll_recv_from(cx, &mut buf))?;
        Poll::Ready(Ok((buf.filled().len(), addr)))
    }

    fn poll_recv(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        let mut buf = ReadBuf::new(buf);
        ready!(self.0.poll_recv(cx, &mut buf))?;
        Poll::Ready(Ok(buf.filled().len()))
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "macos",
        target_os = "solaris",
    ))]
    fn poll_batch_recvmsg<const C: usize>(
        &mut self,
        cx: &mut Context<'_>,
        hdr_v: &mut [RecvMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>> {
        self.0.poll_batch_recvmsg(cx, hdr_v)
    }
}
