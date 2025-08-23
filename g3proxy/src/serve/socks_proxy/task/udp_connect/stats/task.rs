/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_daemon::stat::task::UdpConnectConnectionStats;

use crate::module::udp_connect::UdpConnectTaskRemoteStats;

#[derive(Default)]
pub(crate) struct UdpConnectTaskStats {
    pub(crate) clt: UdpConnectConnectionStats,
    pub(crate) ups: UdpConnectConnectionStats,
}

impl UdpConnectTaskRemoteStats for UdpConnectTaskStats {
    fn add_recv_bytes(&self, size: u64) {
        self.ups.recv.add_bytes(size);
    }

    fn add_recv_packets(&self, n: usize) {
        self.ups.recv.add_packets(n);
    }

    fn add_send_bytes(&self, size: u64) {
        self.ups.send.add_bytes(size);
    }

    fn add_send_packets(&self, n: usize) {
        self.ups.send.add_packets(n);
    }
}
