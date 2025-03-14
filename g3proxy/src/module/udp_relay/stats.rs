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

use std::sync::Arc;

use g3_io_ext::{LimitedRecvStats, LimitedSendStats};

use crate::auth::UserUpstreamTrafficStats;

/// task related stats used at escaper side
pub(crate) trait UdpRelayTaskRemoteStats {
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

pub(crate) type ArcUdpRelayTaskRemoteStats = Arc<dyn UdpRelayTaskRemoteStats + Send + Sync>;

impl UdpRelayTaskRemoteStats for UserUpstreamTrafficStats {
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
pub(crate) struct UdpRelayRemoteWrapperStats {
    all: Vec<ArcUdpRelayTaskRemoteStats>,
}

impl UdpRelayRemoteWrapperStats {
    pub(crate) fn new(
        escaper: ArcUdpRelayTaskRemoteStats,
        task: ArcUdpRelayTaskRemoteStats,
    ) -> Self {
        let mut all = Vec::with_capacity(4);
        all.push(task);
        all.push(escaper);
        UdpRelayRemoteWrapperStats { all }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserUpstreamTrafficStats>>) {
        for s in all {
            self.all.push(s as ArcUdpRelayTaskRemoteStats);
        }
    }
}

impl LimitedRecvStats for UdpRelayRemoteWrapperStats {
    fn add_recv_bytes(&self, size: usize) {
        let size = size as u64;
        self.all.iter().for_each(|stats| stats.add_recv_bytes(size));
    }

    fn add_recv_packets(&self, n: usize) {
        self.all.iter().for_each(|stats| stats.add_recv_packets(n));
    }
}

impl LimitedSendStats for UdpRelayRemoteWrapperStats {
    fn add_send_bytes(&self, size: usize) {
        let size = size as u64;
        self.all.iter().for_each(|stats| stats.add_send_bytes(size));
    }

    fn add_send_packets(&self, n: usize) {
        self.all.iter().for_each(|stats| stats.add_send_packets(n));
    }
}
