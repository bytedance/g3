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
use std::sync::Arc;

use crate::module::udp_relay::{ArcUdpRelayTaskRemoteStats, UdpRelayTaskRemoteStats};

#[derive(Default)]
pub(crate) struct UdpAssociateClientSideHalfStats {
    bytes: u64,
    packets: u64,
}

impl UdpAssociateClientSideHalfStats {
    pub(crate) fn get_bytes(&self) -> u64 {
        self.bytes
    }

    pub(crate) fn get_packets(&self) -> u64 {
        self.packets
    }

    pub(crate) fn add_bytes(&self, size: u64) {
        unsafe {
            let r = &self.bytes as *const u64 as *mut u64;
            *r += size;
        }
    }

    pub(crate) fn add_packet(&self) {
        unsafe {
            let r = &self.packets as *const u64 as *mut u64;
            *r += 1;
        }
    }
}

#[derive(Default)]
pub(crate) struct UdpAssociateClientSideStats {
    pub(crate) recv: UdpAssociateClientSideHalfStats,
    pub(crate) send: UdpAssociateClientSideHalfStats,
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

    fn add_packet(&self) {
        self.packets.fetch_add(1, Ordering::Relaxed);
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

impl UdpAssociateTaskStats {
    #[inline]
    pub(crate) fn for_escaper(self: &Arc<Self>) -> ArcUdpRelayTaskRemoteStats {
        Arc::clone(self) as ArcUdpRelayTaskRemoteStats
    }
}

impl UdpRelayTaskRemoteStats for UdpAssociateTaskStats {
    fn add_recv_bytes(&self, size: u64) {
        self.ups.recv.add_bytes(size);
    }

    fn add_recv_packet(&self) {
        self.ups.recv.add_packet();
    }

    fn add_send_bytes(&self, size: u64) {
        self.ups.send.add_bytes(size);
    }

    fn add_send_packet(&self) {
        self.ups.send.add_packet();
    }
}
