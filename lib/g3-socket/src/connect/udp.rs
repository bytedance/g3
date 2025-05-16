/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
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
