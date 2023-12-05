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
