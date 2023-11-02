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
