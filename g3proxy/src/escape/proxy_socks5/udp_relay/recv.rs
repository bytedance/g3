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
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use tokio::io::{AsyncRead, ReadBuf};

use g3_io_ext::{AsyncUdpRecv, UdpRelayRemoteError, UdpRelayRemoteRecv};
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "macos",
))]
use g3_io_ext::{RecvMsgHdr, UdpRelayPacket, UdpRelayPacketMeta};
use g3_socks::v5::UdpInput;
use g3_types::net::UpstreamAddr;

pub(crate) struct ProxySocks5UdpRelayRemoteRecv<T, C> {
    local_addr: SocketAddr,
    peer_addr: SocketAddr,
    inner: T,
    ctl_stream: C,
    end_on_control_closed: bool,
    ignore_ctl_stream: bool,
}

impl<T, C> ProxySocks5UdpRelayRemoteRecv<T, C>
where
    T: AsyncUdpRecv,
    C: AsyncRead + Unpin,
{
    pub(crate) fn new(
        recv: T,
        local_addr: SocketAddr,
        peer_addr: SocketAddr,
        ctl_stream: C,
        end_on_control_closed: bool,
    ) -> Self {
        ProxySocks5UdpRelayRemoteRecv {
            local_addr,
            peer_addr,
            inner: recv,
            ctl_stream,
            end_on_control_closed,
            ignore_ctl_stream: false,
        }
    }

    fn check_tcp_close(&mut self, cx: &mut Context<'_>) -> Result<(), UdpRelayRemoteError> {
        const MAX_MSG_SIZE: usize = 4;
        let mut buf = [0u8; MAX_MSG_SIZE];

        let mut read_buf = ReadBuf::new(&mut buf);
        match Pin::new(&mut self.ctl_stream).poll_read(cx, &mut read_buf) {
            Poll::Pending => Ok(()),
            Poll::Ready(Ok(_)) => match read_buf.filled().len() {
                0 => {
                    if self.end_on_control_closed {
                        Err(UdpRelayRemoteError::RemoteSessionClosed(
                            self.local_addr,
                            self.peer_addr,
                        ))
                    } else {
                        self.ignore_ctl_stream = true;
                        Ok(())
                    }
                }
                MAX_MSG_SIZE => Err(UdpRelayRemoteError::RemoteSessionError(
                    self.local_addr,
                    self.peer_addr,
                    io::Error::other("unexpected data received in ctl stream"),
                )),
                _ => Ok(()), // drain extra data sent by some bad implementation
            },
            Poll::Ready(Err(e)) => Err(UdpRelayRemoteError::RemoteSessionError(
                self.local_addr,
                self.peer_addr,
                e,
            )),
        }
    }
}

impl<T, C> UdpRelayRemoteRecv for ProxySocks5UdpRelayRemoteRecv<T, C>
where
    T: AsyncUdpRecv,
    C: AsyncRead + Unpin,
{
    fn max_hdr_len(&self) -> usize {
        256 + 4 + 2
    }

    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize, UpstreamAddr), UdpRelayRemoteError>> {
        if self.ignore_ctl_stream {
            self.check_tcp_close(cx)?;
        }

        let nr = ready!(self.inner.poll_recv(cx, buf))
            .map_err(|e| UdpRelayRemoteError::RecvFailed(self.local_addr, e))?;

        let (off, upstream) = UdpInput::parse_header(buf)
            .map_err(|e| UdpRelayRemoteError::InvalidPacket(self.local_addr, e.to_string()))?;

        self.end_on_control_closed = true;
        Poll::Ready(Ok((off, nr, upstream)))
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "macos",
    ))]
    fn poll_recv_packets(
        &mut self,
        cx: &mut Context<'_>,
        packets: &mut [UdpRelayPacket],
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        if self.ignore_ctl_stream {
            self.check_tcp_close(cx)?;
        }

        let mut hdr_v: Vec<RecvMsgHdr<1>> = packets
            .iter_mut()
            .map(|p| RecvMsgHdr::new([io::IoSliceMut::new(p.buf_mut())]))
            .collect();

        let count = ready!(self.inner.poll_batch_recvmsg(cx, &mut hdr_v))
            .map_err(|e| UdpRelayRemoteError::RecvFailed(self.local_addr, e))?;

        let mut r = Vec::with_capacity(count);
        for h in hdr_v.into_iter().take(count) {
            let iov = &h.iov[0];
            let (off, ups) = UdpInput::parse_header(&iov[0..h.n_recv])
                .map_err(|e| UdpRelayRemoteError::InvalidPacket(self.local_addr, e.to_string()))?;
            r.push(UdpRelayPacketMeta::new(iov, off, h.n_recv, ups))
        }
        for (m, p) in r.into_iter().zip(packets.iter_mut()) {
            m.set_packet(p);
        }

        self.end_on_control_closed = true;
        Poll::Ready(Ok(count))
    }
}
