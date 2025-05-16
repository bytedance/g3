/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::{SocketAddr, UdpSocket};

pub(super) struct UdpMetricsSink {
    addr: SocketAddr,
    socket: UdpSocket,
}

impl UdpMetricsSink {
    pub(super) fn new(addr: SocketAddr, socket: UdpSocket) -> Self {
        UdpMetricsSink { addr, socket }
    }

    pub(super) fn send_msg(&self, msg: &[u8]) -> io::Result<usize> {
        self.socket.send_to(msg, self.addr)
    }
}
