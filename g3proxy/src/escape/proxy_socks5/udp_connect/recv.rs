/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use slog::Logger;
use tokio::io::{AsyncRead, ReadBuf};

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
use g3_socks::v5::UdpInput;

pub(crate) struct ProxySocks5UdpConnectRemoteRecv<T, C> {
    inner: T,
    ctl_stream: C,
    end_on_control_closed: bool,
    ignore_ctl_stream: bool,
    logger: Option<Logger>,
}

impl<T, C> ProxySocks5UdpConnectRemoteRecv<T, C>
where
    T: AsyncUdpRecv,
    C: AsyncRead + Unpin,
{
    pub(crate) fn new(
        recv: T,
        ctl_stream: C,
        end_on_control_closed: bool,
        logger: Option<Logger>,
    ) -> Self {
        ProxySocks5UdpConnectRemoteRecv {
            inner: recv,
            ctl_stream,
            end_on_control_closed,
            ignore_ctl_stream: false,
            logger,
        }
    }

    fn check_ctl_stream(&mut self, cx: &mut Context<'_>) -> Result<(), UdpCopyRemoteError> {
        const MAX_MSG_SIZE: usize = 4;
        let mut buf = [0u8; MAX_MSG_SIZE];

        let mut read_buf = ReadBuf::new(&mut buf);
        match Pin::new(&mut self.ctl_stream).poll_read(cx, &mut read_buf) {
            Poll::Pending => Ok(()),
            Poll::Ready(Ok(_)) => match read_buf.filled().len() {
                0 => {
                    if self.end_on_control_closed {
                        Err(UdpCopyRemoteError::RemoteSessionClosed)
                    } else {
                        self.ignore_ctl_stream = true;
                        Ok(())
                    }
                }
                MAX_MSG_SIZE => Err(UdpCopyRemoteError::RemoteSessionError(io::Error::other(
                    "unexpected data received in ctl stream",
                ))),
                _ => Ok(()), // drain extra data sent by some bad implementation
            },
            Poll::Ready(Err(e)) => Err(UdpCopyRemoteError::RemoteSessionError(e)),
        }
    }
}

impl<T, C> UdpCopyRemoteRecv for ProxySocks5UdpConnectRemoteRecv<T, C>
where
    T: AsyncUdpRecv,
    C: AsyncRead + Unpin,
{
    fn error_logger(&self) -> Option<&Logger> {
        self.logger.as_ref()
    }

    fn max_hdr_len(&self) -> usize {
        256 + 4 + 2
    }

    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize), UdpCopyRemoteError>> {
        if !self.ignore_ctl_stream {
            self.check_ctl_stream(cx)?;
        }

        let nr = ready!(self.inner.poll_recv(cx, buf)).map_err(UdpCopyRemoteError::RecvFailed)?;

        let (off, _upstream) = UdpInput::parse_header(buf)
            .map_err(|e| UdpCopyRemoteError::InvalidPacket(e.to_string()))?;

        self.end_on_control_closed = true;
        Poll::Ready(Ok((off, nr)))
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

        if !self.ignore_ctl_stream {
            self.check_ctl_stream(cx)?;
        }

        let mut hdr_v: Vec<RecvMsgHdr<1>> = packets
            .iter_mut()
            .map(|p| RecvMsgHdr::new([io::IoSliceMut::new(p.buf_mut())]))
            .collect();

        let count = ready!(self.inner.poll_batch_recvmsg(cx, &mut hdr_v))
            .map_err(UdpCopyRemoteError::RecvFailed)?;

        let mut r = Vec::with_capacity(count);
        for h in hdr_v.into_iter().take(count) {
            let iov = &h.iov[0];
            let (off, _upstream) = UdpInput::parse_header(&iov[0..h.n_recv])
                .map_err(|e| UdpCopyRemoteError::InvalidPacket(e.to_string()))?;
            r.push(UdpCopyPacketMeta::new(iov, off, h.n_recv));
        }
        for (m, p) in r.into_iter().zip(packets.iter_mut()) {
            m.set_packet(p);
        }

        self.end_on_control_closed = true;
        Poll::Ready(Ok(count))
    }
}
