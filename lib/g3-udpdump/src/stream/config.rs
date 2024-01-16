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

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use g3_types::net::{SocketBufferConfig, UdpMiscSockOpts};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamDumpConfig {
    pub peer: SocketAddr,
    pub buffer: SocketBufferConfig,
    pub opts: UdpMiscSockOpts,
    pub packet_size: usize,
}

impl Default for StreamDumpConfig {
    fn default() -> Self {
        StreamDumpConfig {
            peer: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5555),
            buffer: SocketBufferConfig::default(),
            opts: UdpMiscSockOpts::default(),
            packet_size: 1480,
        }
    }
}
