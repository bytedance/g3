/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::task::{Context, Poll, ready};

use slog::Logger;

use g3_io_ext::{AsyncUdpRecv, UdpCopyRemoteError, UdpCopyRemoteRecv};
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "macos",
))]
use g3_io_ext::{UdpCopyPacket, UdpCopyPacketMeta};

pub(crate) struct DirectUdpConnectRemoteRecv<T> {
    inner: T,
    logger: Option<Logger>,
}

impl<T> DirectUdpConnectRemoteRecv<T>
where
    T: AsyncUdpRecv,
{
    pub(crate) fn new(recv: T, logger: Option<Logger>) -> Self {
        DirectUdpConnectRemoteRecv {
            inner: recv,
            logger,
        }
    }
}

impl<T> UdpCopyRemoteRecv for DirectUdpConnectRemoteRecv<T>
where
    T: AsyncUdpRecv,
{
    fn error_logger(&self) -> Option<&Logger> {
        self.logger.as_ref()
    }

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
        use g3_io_sys::udp::RecvMsgHdr;

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
