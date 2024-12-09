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
