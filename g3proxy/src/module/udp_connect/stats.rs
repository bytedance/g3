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
pub(crate) trait UdpConnectTaskRemoteStats {
    fn add_recv_bytes(&self, size: u64);
    fn add_recv_packet(&self) {
        self.add_recv_packets(1);
    }
    fn add_recv_packets(&self, n: usize);
    fn add_send_bytes(&self, size: u64);
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
pub(crate) struct UdpConnectRemoteWrapperStats<T> {
    escaper: Arc<T>,
    task: ArcUdpConnectTaskRemoteStats,
    others: Vec<ArcUdpConnectTaskRemoteStats>,
}

impl<T: UdpConnectTaskRemoteStats> UdpConnectRemoteWrapperStats<T> {
    pub(crate) fn new(escaper: &Arc<T>, task: ArcUdpConnectTaskRemoteStats) -> Self {
        UdpConnectRemoteWrapperStats {
            escaper: Arc::clone(escaper),
            task,
            others: Vec::with_capacity(2),
        }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserUpstreamTrafficStats>>) {
        for s in all {
            self.others.push(s as ArcUdpConnectTaskRemoteStats);
        }
    }
}

impl<T: UdpConnectTaskRemoteStats> LimitedRecvStats for UdpConnectRemoteWrapperStats<T> {
    fn add_recv_bytes(&self, size: usize) {
        let size = size as u64;
        self.escaper.add_recv_bytes(size);
        self.task.add_recv_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_recv_bytes(size));
    }

    fn add_recv_packets(&self, n: usize) {
        self.escaper.add_recv_packets(n);
        self.task.add_recv_packets(n);
        self.others
            .iter()
            .for_each(|stats| stats.add_recv_packets(n));
    }
}

impl<T: UdpConnectTaskRemoteStats> LimitedSendStats for UdpConnectRemoteWrapperStats<T> {
    fn add_send_bytes(&self, size: usize) {
        let size = size as u64;
        self.escaper.add_send_bytes(size);
        self.task.add_send_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_send_bytes(size));
    }

    fn add_send_packets(&self, n: usize) {
        self.escaper.add_send_packets(n);
        self.task.add_send_packets(n);
        self.others
            .iter()
            .for_each(|stats| stats.add_send_packets(n));
    }
}
