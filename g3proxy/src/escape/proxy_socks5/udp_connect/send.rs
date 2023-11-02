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
use std::task::{ready, Context, Poll};

use g3_io_ext::{AsyncUdpSend, UdpCopyRemoteError, UdpCopyRemoteSend};
use g3_socks::v5::UdpOutput;
use g3_types::net::UpstreamAddr;

pub(crate) struct ProxySocks5UdpConnectRemoteSend<T> {
    upstream: UpstreamAddr,
    inner: T,
}

impl<T> ProxySocks5UdpConnectRemoteSend<T>
where
    T: AsyncUdpSend,
{
    pub(crate) fn new(send: T, upstream: UpstreamAddr) -> Self {
        ProxySocks5UdpConnectRemoteSend {
            upstream,
            inner: send,
        }
    }

    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, UdpCopyRemoteError>> {
        const STATIC_BUF_LEN: usize = 128;

        let header_len = UdpOutput::calc_header_len(&self.upstream);
        let nw = if header_len <= STATIC_BUF_LEN {
            let mut hdr_buf = [0u8; STATIC_BUF_LEN];
            UdpOutput::generate_header(&mut hdr_buf, &self.upstream);
            ready!(self.inner.poll_sendmsg(
                cx,
                &[IoSlice::new(&hdr_buf[0..header_len]), IoSlice::new(buf)],
                None
            ))
            .map_err(UdpCopyRemoteError::SendFailed)?
        } else {
            let mut hdr_buf = vec![0u8; header_len];
            UdpOutput::generate_header(&mut hdr_buf, &self.upstream);
            ready!(self.inner.poll_sendmsg(
                cx,
                &[IoSlice::new(&hdr_buf[0..header_len]), IoSlice::new(buf)],
                None
            ))
            .map_err(UdpCopyRemoteError::SendFailed)?
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
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, UdpCopyRemoteError>> {
        self.poll_send_packet(cx, buf)
    }
}
