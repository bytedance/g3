/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;

use log::trace;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

const UDP_BATCH_SEND_SIZE: usize = 8;

pub(super) struct Sinker {
    receiver: mpsc::UnboundedReceiver<Vec<u8>>,
    socket: UdpSocket,
}

impl Sinker {
    pub(super) fn new(receiver: mpsc::UnboundedReceiver<Vec<u8>>, socket: UdpSocket) -> Self {
        Sinker { receiver, socket }
    }

    pub(super) async fn into_running(mut self) {
        let mut buf = Vec::with_capacity(UDP_BATCH_SEND_SIZE);
        loop {
            let nr = self.receiver.recv_many(&mut buf, UDP_BATCH_SEND_SIZE).await;
            if nr == 0 {
                break;
            }

            if let Err(e) = self.send_udp(&buf).await {
                trace!("stream dump udp send error: {e}");
            }
            buf.clear();
        }
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    async fn send_udp(&self, packets: &[Vec<u8>]) -> io::Result<()> {
        use g3_io_ext::{SendMsgHdr, UdpSocketExt};
        use std::future::poll_fn;
        use std::io::IoSlice;

        let mut msgs: Vec<_> = packets
            .iter()
            .map(|v| SendMsgHdr::new([IoSlice::new(v.as_slice())], None))
            .collect();
        let mut offset = 0;
        while offset < msgs.len() {
            offset += poll_fn(|cx| self.socket.poll_batch_sendmsg(cx, &mut msgs[offset..])).await?;
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn send_udp(&self, packets: &[Vec<u8>]) -> io::Result<()> {
        use g3_io_ext::{SendMsgHdr, UdpSocketExt};
        use std::future::poll_fn;
        use std::io::IoSlice;

        let mut msgs: Vec<_> = packets
            .iter()
            .map(|v| SendMsgHdr::new([IoSlice::new(v.as_slice())], None))
            .collect();
        let mut offset = 0;
        while offset < msgs.len() {
            offset +=
                poll_fn(|cx| self.socket.poll_batch_sendmsg_x(cx, &mut msgs[offset..])).await?;
        }
        Ok(())
    }

    #[cfg(any(windows, target_os = "dragonfly", target_os = "illumos"))]
    async fn send_udp(&self, packets: &[Vec<u8>]) -> io::Result<()> {
        for pkt in packets {
            self.socket.send(pkt.as_slice()).await?;
        }
        Ok(())
    }
}
