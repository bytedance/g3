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
use g3_socks::v5::UdpOutput;
use g3_types::net::UpstreamAddr;

pub(crate) struct ProxySocks5UdpRelayRemoteSend<T> {
    local_addr: SocketAddr,
    peer_addr: SocketAddr,
    inner: T,
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
        }
    }

    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        to: &UpstreamAddr,
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        const STATIC_BUF_LEN: usize = 128;
        let header_len = UdpOutput::calc_header_len(to);
        let nw = if header_len <= STATIC_BUF_LEN {
            let mut hdr_buf = [0u8; STATIC_BUF_LEN];
            UdpOutput::generate_header(&mut hdr_buf, to);
            ready!(self.inner.poll_sendmsg(
                cx,
                &[IoSlice::new(&hdr_buf[0..header_len]), IoSlice::new(buf)],
                None
            ))
            .map_err(|e| UdpRelayRemoteError::SendFailed(self.local_addr, self.peer_addr, e))?
        } else {
            let mut hdr_buf = vec![0u8; header_len];
            UdpOutput::generate_header(&mut hdr_buf, to);
            ready!(self
                .inner
                .poll_sendmsg(cx, &[IoSlice::new(&hdr_buf), IoSlice::new(buf)], None))
            .map_err(|e| UdpRelayRemoteError::SendFailed(self.local_addr, self.peer_addr, e))?
        };
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
        self.poll_send_packet(cx, buf, to)
    }
}
