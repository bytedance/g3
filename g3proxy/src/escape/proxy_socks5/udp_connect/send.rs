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
use std::task::{ready, Context, Poll};

use bytes::BufMut;

use g3_io_ext::{AsyncUdpSend, UdpCopyRemoteError, UdpCopyRemoteSend};
use g3_socks::v5::UdpOutput;
use g3_types::net::UpstreamAddr;

pub(super) struct ProxySocks5UdpConnectRemoteSend<T> {
    upstream: UpstreamAddr,
    inner: T,
}

impl<T> ProxySocks5UdpConnectRemoteSend<T>
where
    T: AsyncUdpSend,
{
    pub(super) fn new(send: T, upstream: UpstreamAddr) -> Self {
        ProxySocks5UdpConnectRemoteSend {
            upstream,
            inner: send,
        }
    }

    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        buf_off: usize,
        buf_len: usize,
    ) -> Poll<Result<usize, UdpCopyRemoteError>> {
        let header_len = UdpOutput::calc_header_len(&self.upstream);
        let nw = if header_len <= buf_off {
            UdpOutput::generate_header(&mut buf[buf_off - header_len..buf_off], &self.upstream);
            ready!(self
                .inner
                .poll_send(cx, &buf[buf_off - header_len..buf_len]))
            .map_err(UdpCopyRemoteError::SendFailed)?
        } else {
            let mut new_buf: Vec<u8> = Vec::with_capacity(buf_len - buf_off + header_len);
            UdpOutput::generate_header(&mut new_buf[0..header_len], &self.upstream);
            unsafe { new_buf.set_len(header_len) };
            new_buf.put_slice(&buf[buf_off..buf_len]);
            ready!(self.inner.poll_send(cx, &new_buf)).map_err(UdpCopyRemoteError::SendFailed)?
        };
        if nw == 0 {
            Poll::Ready(Err(UdpCopyRemoteError::SendFailed(io::Error::new(
                io::ErrorKind::WriteZero,
                "write zero byte into sender",
            ))))
        } else {
            Poll::Ready(Ok(nw))
        }
    }
}

impl<T> UdpCopyRemoteSend for ProxySocks5UdpConnectRemoteSend<T>
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
    ) -> Poll<Result<usize, UdpCopyRemoteError>> {
        self.poll_send_packet(cx, buf, buf_off, buf_len)
    }
}
