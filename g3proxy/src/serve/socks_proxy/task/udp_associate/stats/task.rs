/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::atomic::{AtomicU64, Ordering};

use g3_daemon::stat::task::UdpConnectHalfConnectionStats;

use crate::module::udp_relay::UdpRelayTaskRemoteStats;

#[derive(Default)]
pub(crate) struct UdpAssociateClientSideStats {
    pub(crate) recv: UdpConnectHalfConnectionStats,
    pub(crate) send: UdpConnectHalfConnectionStats,
}

#[derive(Default)]
pub(crate) struct UdpAssociateRemoteSideHalfStats {
    bytes: AtomicU64,
    packets: AtomicU64,
}

impl UdpAssociateRemoteSideHalfStats {
    pub(crate) fn get_bytes(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed)
    }

    pub(crate) fn get_packets(&self) -> u64 {
        self.packets.load(Ordering::Relaxed)
    }

    fn add_bytes(&self, size: u64) {
        self.bytes.fetch_add(size, Ordering::Relaxed);
    }

    fn add_packets(&self, n: usize) {
        self.packets.fetch_add(n as u64, Ordering::Relaxed);
    }
}

#[derive(Default)]
pub(crate) struct UdpAssociateRemoteSideStats {
    pub(crate) recv: UdpAssociateRemoteSideHalfStats,
    pub(crate) send: UdpAssociateRemoteSideHalfStats,
}

#[derive(Default)]
pub(crate) struct UdpAssociateTaskStats {
    pub(crate) clt: UdpAssociateClientSideStats,
    pub(crate) ups: UdpAssociateRemoteSideStats,
}

impl UdpRelayTaskRemoteStats for UdpAssociateTaskStats {
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
