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
use crate::module::udp_relay::{ArcUdpRelayTaskRemoteStats, UdpRelayTaskRemoteStats};

#[derive(Clone)]
pub(crate) struct ProxySocks5UdpRelayRemoteStats<T> {
    escaper: Arc<T>,
    task: ArcUdpRelayTaskRemoteStats,
    others: Vec<ArcUdpRelayTaskRemoteStats>,
}

impl<T> ProxySocks5UdpRelayRemoteStats<T>
where
    T: UdpRelayTaskRemoteStats + Send + Sync + 'static,
{
    pub(crate) fn new(escaper: &Arc<T>, task: ArcUdpRelayTaskRemoteStats) -> Self {
        ProxySocks5UdpRelayRemoteStats {
            escaper: Arc::clone(escaper),
            task,
            others: Vec::with_capacity(2),
        }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserUpstreamTrafficStats>>) {
        for s in all {
            self.others.push(s as ArcUdpRelayTaskRemoteStats);
        }
    }
}

impl<T: UdpRelayTaskRemoteStats> LimitedRecvStats for ProxySocks5UdpRelayRemoteStats<T> {
    fn add_recv_bytes(&self, size: usize) {
        let size = size as u64;
        self.escaper.add_recv_bytes(size);
        self.task.add_recv_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_recv_bytes(size));
    }

    fn add_recv_packet(&self) {
        self.escaper.add_recv_packet();
        self.task.add_recv_packet();
        self.others.iter().for_each(|stats| stats.add_recv_packet());
    }
}

impl<T: UdpRelayTaskRemoteStats> LimitedSendStats for ProxySocks5UdpRelayRemoteStats<T> {
    fn add_send_bytes(&self, size: usize) {
        let size = size as u64;
        self.escaper.add_send_bytes(size);
        self.task.add_send_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_send_bytes(size));
    }

    fn add_send_packet(&self) {
        self.escaper.add_send_packet();
        self.task.add_send_packet();
        self.others.iter().for_each(|stats| stats.add_send_packet());
    }
}
