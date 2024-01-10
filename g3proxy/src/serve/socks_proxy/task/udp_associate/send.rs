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

use g3_io_ext::{AsyncUdpSend, UdpRelayClientError, UdpRelayClientSend};
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

pub(super) struct Socks5UdpAssociateClientSend<T> {
    inner: T,
    client: SocketAddr,
    socks_headers: Vec<SocksUdpHeader>,
}

impl<T> Socks5UdpAssociateClientSend<T>
where
    T: AsyncUdpSend,
{
    pub(super) fn new(inner: T, client: SocketAddr) -> Self {
        Socks5UdpAssociateClientSend {
            inner,
            client,
            socks_headers: vec![SocksUdpHeader::default(); 4],
        }
    }
}

impl<T> UdpRelayClientSend for Socks5UdpAssociateClientSend<T>
where
    T: AsyncUdpSend + Send,
{
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        from: &UpstreamAddr,
    ) -> Poll<Result<usize, UdpRelayClientError>> {
        let socks_header = self.socks_headers.get_mut(0).unwrap();
        let nw = ready!(self.inner.poll_sendmsg(
            cx,
            &[IoSlice::new(socks_header.encode(from)), IoSlice::new(buf)],
            Some(self.client)
        ))
        .map_err(UdpRelayClientError::SendFailed)?;
        if nw == 0 {
            Poll::Ready(Err(UdpRelayClientError::SendFailed(io::Error::new(
                io::ErrorKind::WriteZero,
                "write zero byte into sender",
            ))))
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
    ) -> Poll<Result<usize, UdpRelayClientError>> {
        if packets.len() > self.socks_headers.len() {
            self.socks_headers.resize(packets.len(), Default::default());
        }
        let mut msgs = Vec::with_capacity(packets.len());
        for (p, h) in packets.iter().zip(self.socks_headers.iter_mut()) {
            msgs.push(SendMsgHdr {
                iov: [
                    IoSlice::new(h.encode(p.upstream())),
                    IoSlice::new(p.payload()),
                ],
                addr: None,
            });
        }

        let count = ready!(self.inner.poll_batch_sendmsg(cx, &msgs))
            .map_err(UdpRelayClientError::SendFailed)?;
        if count == 0 {
            Poll::Ready(Err(UdpRelayClientError::SendFailed(io::Error::new(
                io::ErrorKind::WriteZero,
                "write zero packet into sender",
            ))))
        } else {
            Poll::Ready(Ok(count))
        }
    }
}
