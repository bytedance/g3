/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::{SocketAddr, UdpSocket};

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "solaris",
))]
use g3_io_sys::udp::{SendMsgHdr, UdpSocketExt};

use super::SinkBuf;

pub(super) struct UdpMetricsSink {
    addr: SocketAddr,
    socket: UdpSocket,
    max_segment_size: usize,
}

impl UdpMetricsSink {
    pub(super) fn new(
        addr: SocketAddr,
        socket: UdpSocket,
        max_segment_size: Option<usize>,
    ) -> Self {
        UdpMetricsSink {
            addr,
            socket,
            max_segment_size: max_segment_size.unwrap_or(1400),
        }
    }

    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    )))]
    pub(super) fn send_batch(&self, buf: &mut SinkBuf) -> io::Result<()> {
        for packet in buf.iter(self.max_segment_size) {
            self.socket.send_to(packet.as_ref(), self.addr)?;
        }
        Ok(())
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    pub(super) fn send_batch(&self, buf: &mut SinkBuf) -> io::Result<()> {
        const MAX_BATCH_SIZE: usize = 32;

        let mut hdrs: [SendMsgHdr<'_, 1>; MAX_BATCH_SIZE] = unsafe { std::mem::zeroed() };

        let mut offset = 0usize;
        for packet in buf.iter(self.max_segment_size) {
            hdrs[offset] = SendMsgHdr::new([packet], Some(self.addr));
            offset += 1;
            if offset >= MAX_BATCH_SIZE {
                self.socket.batch_sendmsg(&mut hdrs)?;
                offset = 0;
            }
        }
        if offset > 0 {
            self.socket.batch_sendmsg(&mut hdrs[..offset])?;
        }
        Ok(())
    }
}
