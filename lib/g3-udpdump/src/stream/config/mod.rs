/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use g3_types::net::{SocketBufferConfig, UdpMiscSockOpts};

#[cfg(feature = "yaml")]
mod yaml;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamDumpConfig {
    pub peer: SocketAddr,
    pub buffer: SocketBufferConfig,
    pub opts: UdpMiscSockOpts,
    pub packet_size: usize,
    pub client_side: bool,
}

impl Default for StreamDumpConfig {
    fn default() -> Self {
        StreamDumpConfig {
            peer: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5555),
            buffer: SocketBufferConfig::default(),
            opts: UdpMiscSockOpts::default(),
            packet_size: 1480,
            client_side: false,
        }
    }
}
