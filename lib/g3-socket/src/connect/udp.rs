/*
 * Copyright 2025 ByteDance and/or its affiliates.
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
use std::net::{SocketAddr, UdpSocket};

use g3_types::net::{SocketBufferConfig, UdpMiscSockOpts};

use crate::BindAddr;

#[derive(Clone)]
pub struct UdpConnectInfo {
    pub server: SocketAddr,
    pub bind: BindAddr,
    pub buf_conf: SocketBufferConfig,
    pub misc_opts: UdpMiscSockOpts,
}

impl UdpConnectInfo {
    pub fn udp_connect(&self) -> io::Result<UdpSocket> {
        let socket =
            crate::udp::new_std_socket_to(self.server, &self.bind, self.buf_conf, self.misc_opts)?;
        socket.connect(self.server)?;
        Ok(socket)
    }
}
