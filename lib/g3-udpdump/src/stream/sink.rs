/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use log::debug;
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

            if let Err(e) = self.send_udp(&buf[0..nr]).await {
                debug!("stream dump udp send error: {e}");
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

    #[cfg(any(target_os = "macos", target_os = "dragonfly"))]
    async fn send_udp(&self, packets: &[Vec<u8>]) -> io::Result<()> {
        for pkt in packets {
            self.socket.send(pkt.as_slice()).await?;
        }
        Ok(())
    }
}
