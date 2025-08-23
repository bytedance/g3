/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_io_ext::{LimitedRecvStats, LimitedSendStats};

use crate::auth::UserUpstreamTrafficStats;

/// task related stats used at escaper side
pub(crate) trait UdpConnectTaskRemoteStats {
    fn add_recv_bytes(&self, size: u64);
    #[allow(unused)]
    fn add_recv_packet(&self) {
        self.add_recv_packets(1);
    }
    fn add_recv_packets(&self, n: usize);
    fn add_send_bytes(&self, size: u64);
    #[allow(unused)]
    fn add_send_packet(&self) {
        self.add_send_packets(1);
    }
    fn add_send_packets(&self, n: usize);
}

pub(crate) type ArcUdpConnectTaskRemoteStats = Arc<dyn UdpConnectTaskRemoteStats + Send + Sync>;

impl UdpConnectTaskRemoteStats for UserUpstreamTrafficStats {
    fn add_recv_bytes(&self, size: u64) {
        self.io.udp.add_in_bytes(size);
    }

    fn add_recv_packets(&self, n: usize) {
        self.io.udp.add_in_packets(n);
    }

    fn add_send_bytes(&self, size: u64) {
        self.io.udp.add_out_bytes(size);
    }

    fn add_send_packets(&self, n: usize) {
        self.io.udp.add_out_packets(n);
    }
}

#[derive(Clone)]
pub(crate) struct UdpConnectRemoteWrapperStats {
    all: Vec<ArcUdpConnectTaskRemoteStats>,
}

impl UdpConnectRemoteWrapperStats {
    pub(crate) fn new(
        escaper: ArcUdpConnectTaskRemoteStats,
        task: ArcUdpConnectTaskRemoteStats,
    ) -> Self {
        let mut all = Vec::with_capacity(4);
        all.push(task);
        all.push(escaper);
        UdpConnectRemoteWrapperStats { all }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserUpstreamTrafficStats>>) {
        for s in all {
            self.all.push(s as ArcUdpConnectTaskRemoteStats);
        }
    }
}

impl LimitedRecvStats for UdpConnectRemoteWrapperStats {
    fn add_recv_bytes(&self, size: usize) {
        let size = size as u64;
        self.all.iter().for_each(|stats| stats.add_recv_bytes(size));
    }

    fn add_recv_packets(&self, n: usize) {
        self.all.iter().for_each(|stats| stats.add_recv_packets(n));
    }
}

impl LimitedSendStats for UdpConnectRemoteWrapperStats {
    fn add_send_bytes(&self, size: usize) {
        let size = size as u64;
        self.all.iter().for_each(|stats| stats.add_send_bytes(size));
    }

    fn add_send_packets(&self, n: usize) {
        self.all.iter().for_each(|stats| stats.add_send_packets(n));
    }
}
