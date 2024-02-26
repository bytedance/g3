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

use g3_io_ext::{AsyncUdpSend, UdpCopyClientError, UdpCopyClientSend};
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
))]
use g3_io_ext::{SendMsgHdr, UdpCopyPacket};
use g3_socks::v5::UdpOutput;
use g3_types::net::UpstreamAddr;

pub(super) struct Socks5UdpConnectClientSend<T> {
    inner: T,
    socks5_header: Vec<u8>,
}

impl<T> Socks5UdpConnectClientSend<T>
where
    T: AsyncUdpSend,
{
    pub(super) fn new(inner: T, upstream: UpstreamAddr) -> Self {
        let header_len = UdpOutput::calc_header_len(&upstream);
        let mut socks5_header = vec![0; header_len];
        UdpOutput::generate_header(&mut socks5_header, &upstream);
        Socks5UdpConnectClientSend {
            inner,
            socks5_header,
        }
    }
}

impl<T> UdpCopyClientSend for Socks5UdpConnectClientSend<T>
where
    T: AsyncUdpSend + Send,
{
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, UdpCopyClientError>> {
        let nw = ready!(self.inner.poll_sendmsg(
            cx,
            &[IoSlice::new(&self.socks5_header), IoSlice::new(buf)],
            None
        ))
        .map_err(UdpCopyClientError::SendFailed)?;
        if nw == 0 {
            Poll::Ready(Err(UdpCopyClientError::SendFailed(io::Error::new(
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
        packets: &[UdpCopyPacket],
    ) -> Poll<Result<usize, UdpCopyClientError>> {
        let mut msgs: Vec<SendMsgHdr<2>> = packets
            .iter()
            .map(|p| {
                SendMsgHdr::new(
                    [IoSlice::new(&self.socks5_header), IoSlice::new(p.payload())],
                    None,
                )
            })
            .collect();

        let count = ready!(self.inner.poll_batch_sendmsg(cx, &mut msgs))
            .map_err(UdpCopyClientError::SendFailed)?;
        if count == 0 {
            Poll::Ready(Err(UdpCopyClientError::SendFailed(io::Error::new(
                io::ErrorKind::WriteZero,
                "write zero packet into sender",
            ))))
        } else {
            Poll::Ready(Ok(count))
        }
    }
}
