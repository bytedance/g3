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

use std::io;
use std::net::SocketAddr;
use std::task::{ready, Context, Poll};

use futures_util::FutureExt;
use tokio::sync::oneshot;

use g3_io_ext::{AsyncUdpRecv, UdpRelayRemoteError, UdpRelayRemoteRecv};
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
))]
use g3_io_ext::{RecvMsgHdr, UdpRelayPacket};
use g3_socks::v5::UdpInput;
use g3_types::net::UpstreamAddr;

pub(crate) struct ProxySocks5UdpRelayRemoteRecv<T> {
    local_addr: SocketAddr,
    peer_addr: SocketAddr,
    inner: T,
    tcp_close_receiver: oneshot::Receiver<Option<io::Error>>,
}

impl<T> ProxySocks5UdpRelayRemoteRecv<T>
where
    T: AsyncUdpRecv,
{
    pub(crate) fn new(
        recv: T,
        local_addr: SocketAddr,
        peer_addr: SocketAddr,
        tcp_close_receiver: oneshot::Receiver<Option<io::Error>>,
    ) -> Self {
        ProxySocks5UdpRelayRemoteRecv {
            local_addr,
            peer_addr,
            inner: recv,
            tcp_close_receiver,
        }
    }

    fn check_tcp_close(&mut self, cx: &mut Context<'_>) -> Result<(), UdpRelayRemoteError> {
        match self.tcp_close_receiver.poll_unpin(cx) {
            Poll::Pending => Ok(()),
            Poll::Ready(Ok(None)) => Err(UdpRelayRemoteError::RemoteSessionClosed(
                self.local_addr,
                self.peer_addr,
            )),
            Poll::Ready(Ok(Some(e))) => Err(UdpRelayRemoteError::RemoteSessionError(
                self.local_addr,
                self.peer_addr,
                e,
            )),
            Poll::Ready(Err(_)) => Err(UdpRelayRemoteError::InternalServerError(
                "tcp close wait channel closed unexpected",
            )),
        }
    }
}

impl<T> UdpRelayRemoteRecv for ProxySocks5UdpRelayRemoteRecv<T>
where
    T: AsyncUdpRecv,
{
    fn max_hdr_len(&self) -> usize {
        256 + 4 + 2
    }

    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize, UpstreamAddr), UdpRelayRemoteError>> {
        self.check_tcp_close(cx)?;

        let nr = ready!(self.inner.poll_recv(cx, buf))
            .map_err(|e| UdpRelayRemoteError::RecvFailed(self.local_addr, e))?;

        let (off, upstream) = UdpInput::parse_header(buf)
            .map_err(|e| UdpRelayRemoteError::InvalidPacket(self.local_addr, e.to_string()))?;
        Poll::Ready(Ok((off, nr, upstream)))
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
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        use std::io::IoSliceMut;

        self.check_tcp_close(cx)?;

        let mut hdr_v: Vec<RecvMsgHdr<1>> = packets
            .iter_mut()
            .map(|p| RecvMsgHdr::new([IoSliceMut::new(p.buf_mut())]))
            .collect();

        let count = ready!(self.inner.poll_batch_recvmsg(cx, &mut hdr_v))
            .map_err(|e| UdpRelayRemoteError::RecvFailed(self.local_addr, e))?;

        let mut r = Vec::with_capacity(count);
        for h in hdr_v.into_iter().take(count) {
            let iov = &h.iov[0];
            let (off, ups) = UdpInput::parse_header(&iov[0..h.n_recv])
                .map_err(|e| UdpRelayRemoteError::InvalidPacket(self.local_addr, e.to_string()))?;
            r.push((off, h.n_recv, ups))
        }

        for ((off, l, ups), p) in r.into_iter().zip(packets.iter_mut()) {
            p.set_offset(off);
            p.set_length(l);
            p.set_upstream(ups);
        }

        Poll::Ready(Ok(count))
    }
}
