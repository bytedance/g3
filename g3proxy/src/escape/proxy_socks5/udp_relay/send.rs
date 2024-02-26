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

use std::io::{self, IoSlice};
use std::net::SocketAddr;
use std::task::{ready, Context, Poll};

use g3_io_ext::{AsyncUdpSend, UdpRelayRemoteError, UdpRelayRemoteSend};
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
))]
use g3_io_ext::{SendMsgHdr, UdpRelayPacket};
use g3_socks::v5::SocksUdpHeader;
use g3_types::net::UpstreamAddr;

pub(crate) struct ProxySocks5UdpRelayRemoteSend<T> {
    local_addr: SocketAddr,
    peer_addr: SocketAddr,
    inner: T,
    socks_headers: Vec<SocksUdpHeader>,
}

impl<T> ProxySocks5UdpRelayRemoteSend<T>
where
    T: AsyncUdpSend,
{
    pub(crate) fn new(send: T, local_addr: SocketAddr, peer_addr: SocketAddr) -> Self {
        ProxySocks5UdpRelayRemoteSend {
            local_addr,
            peer_addr,
            inner: send,
            socks_headers: vec![SocksUdpHeader::default(); 4],
        }
    }
}

impl<T> UdpRelayRemoteSend for ProxySocks5UdpRelayRemoteSend<T>
where
    T: AsyncUdpSend + Send,
{
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        to: &UpstreamAddr,
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        let socks_header = self.socks_headers.get_mut(0).unwrap();
        let nw = ready!(self.inner.poll_sendmsg(
            cx,
            &[IoSlice::new(socks_header.encode(to)), IoSlice::new(buf)],
            None
        ))
        .map_err(|e| UdpRelayRemoteError::SendFailed(self.local_addr, self.peer_addr, e))?;
        if nw == 0 {
            Poll::Ready(Err(UdpRelayRemoteError::SendFailed(
                self.local_addr,
                self.peer_addr,
                io::Error::new(io::ErrorKind::WriteZero, "write zero byte into sender"),
            )))
        } else {
            Poll::Ready(Ok(nw))
        }
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
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        if packets.len() > self.socks_headers.len() {
            self.socks_headers.resize(packets.len(), Default::default());
        }
        let mut msgs = Vec::with_capacity(packets.len());
        for (p, h) in packets.iter().zip(self.socks_headers.iter_mut()) {
            msgs.push(SendMsgHdr::new(
                [
                    IoSlice::new(h.encode(p.upstream())),
                    IoSlice::new(p.payload()),
                ],
                None,
            ));
        }

        let count = ready!(self.inner.poll_batch_sendmsg(cx, &mut msgs))
            .map_err(|e| UdpRelayRemoteError::SendFailed(self.local_addr, self.peer_addr, e))?;
        if count == 0 {
            Poll::Ready(Err(UdpRelayRemoteError::SendFailed(
                self.local_addr,
                self.peer_addr,
                io::Error::new(io::ErrorKind::WriteZero, "write zero packet into sender"),
            )))
        } else {
            Poll::Ready(Ok(count))
        }
    }
}
