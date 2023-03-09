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

use bytes::BufMut;

use g3_io_ext::{AsyncUdpSend, UdpRelayRemoteError, UdpRelayRemoteSend};
use g3_socks::v5::UdpOutput;
use g3_types::net::UpstreamAddr;

pub(super) struct ProxySocks5UdpRelayRemoteSend<T> {
    local_addr: SocketAddr,
    peer_addr: SocketAddr,
    inner: T,
}

impl<T> ProxySocks5UdpRelayRemoteSend<T>
where
    T: AsyncUdpSend,
{
    pub(super) fn new(send: T, local_addr: SocketAddr, peer_addr: SocketAddr) -> Self {
        ProxySocks5UdpRelayRemoteSend {
            local_addr,
            peer_addr,
            inner: send,
        }
    }

    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        buf_off: usize,
        buf_len: usize,
        to: &UpstreamAddr,
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        let header_len = UdpOutput::calc_header_len(to);
        let nw = if header_len <= buf_off {
            UdpOutput::generate_header(&mut buf[buf_off - header_len..buf_off], to);
            ready!(self
                .inner
                .poll_send(cx, &buf[buf_off - header_len..buf_len]))
            .map_err(|e| UdpRelayRemoteError::SendFailed(self.local_addr, self.peer_addr, e))?
        } else {
            let mut new_buf: Vec<u8> = Vec::with_capacity(buf_len - buf_off + header_len);
            UdpOutput::generate_header(&mut new_buf[0..header_len], to);
            unsafe { new_buf.set_len(header_len) }
            new_buf.put_slice(&buf[buf_off..buf_len]);
            ready!(self.inner.poll_send(cx, &new_buf))
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
    fn buf_reserve_length(&self) -> usize {
        256 + 4 + 2
    }

    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        buf_off: usize,
        buf_len: usize,
        to: &UpstreamAddr,
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        self.poll_send_packet(cx, buf, buf_off, buf_len, to)
    }
}
