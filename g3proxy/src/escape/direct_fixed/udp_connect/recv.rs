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

use std::task::{Context, Poll, ready};

use g3_io_ext::{AsyncUdpRecv, UdpCopyRemoteError, UdpCopyRemoteRecv};
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "macos",
))]
use g3_io_ext::{RecvMsgHdr, UdpCopyPacket, UdpCopyPacketMeta};

pub(crate) struct DirectUdpConnectRemoteRecv<T> {
    inner: T,
}

impl<T> DirectUdpConnectRemoteRecv<T>
where
    T: AsyncUdpRecv,
{
    pub(crate) fn new(recv: T) -> Self {
        DirectUdpConnectRemoteRecv { inner: recv }
    }
}

impl<T> UdpCopyRemoteRecv for DirectUdpConnectRemoteRecv<T>
where
    T: AsyncUdpRecv,
{
    fn max_hdr_len(&self) -> usize {
        0
    }

    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize), UdpCopyRemoteError>> {
        let nr = ready!(self.inner.poll_recv(cx, buf)).map_err(UdpCopyRemoteError::RecvFailed)?;
        Poll::Ready(Ok((0, nr)))
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
        packets: &mut [UdpCopyPacket],
    ) -> Poll<Result<usize, UdpCopyRemoteError>> {
        let mut hdr_v: Vec<RecvMsgHdr<1>> = packets
            .iter_mut()
            .map(|p| RecvMsgHdr::new([std::io::IoSliceMut::new(p.buf_mut())]))
            .collect();

        let count = ready!(self.inner.poll_batch_recvmsg(cx, &mut hdr_v))
            .map_err(UdpCopyRemoteError::RecvFailed)?;

        let mut r = Vec::with_capacity(count);
        for h in hdr_v.into_iter().take(count) {
            r.push(UdpCopyPacketMeta::new(&h.iov[0], 0, h.n_recv));
        }
        for (m, p) in r.into_iter().zip(packets.iter_mut()) {
            m.set_packet(p);
        }

        Poll::Ready(Ok(count))
    }
}
