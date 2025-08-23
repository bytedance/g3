/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::SocketAddr;

use tokio::net::UdpSocket;

use g3_types::net::UdpListenConfig;

pub(crate) struct UdpDgramIo {
    socket: UdpSocket,
}

impl UdpDgramIo {
    pub(crate) fn new(config: &UdpListenConfig) -> io::Result<Self> {
        let socket = g3_socket::udp::new_std_bind_listen(config)?;
        Ok(UdpDgramIo {
            socket: UdpSocket::from_std(socket)?,
        })
    }

    pub(crate) async fn recv_req(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.socket.recv_from(buf).await
    }

    pub(crate) async fn send_rsp(&self, data: &[u8], peer: SocketAddr) -> io::Result<()> {
        let nw = self.socket.send_to(data, peer).await?;
        if nw != data.len() {
            Err(io::Error::other(format!(
                "not all data written, only {nw}/{}",
                data.len()
            )))
        } else {
            Ok(())
        }
    }
}
