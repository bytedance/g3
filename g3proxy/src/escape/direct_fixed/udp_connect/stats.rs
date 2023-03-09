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

use g3_io_ext::{ArcLimitedRecvStats, ArcLimitedSendStats, LimitedRecvStats, LimitedSendStats};

use super::DirectFixedEscaperStats;
use crate::auth::UserUpstreamTrafficStats;
use crate::module::udp_connect::ArcUdpConnectTaskRemoteStats;

#[derive(Clone)]
pub(super) struct DirectUdpConnectRemoteStats {
    escaper: Arc<DirectFixedEscaperStats>,
    task: ArcUdpConnectTaskRemoteStats,
    others: Vec<ArcUdpConnectTaskRemoteStats>,
}

impl DirectUdpConnectRemoteStats {
    pub(super) fn new(
        escaper: &Arc<DirectFixedEscaperStats>,
        task: ArcUdpConnectTaskRemoteStats,
    ) -> Self {
        DirectUdpConnectRemoteStats {
            escaper: Arc::clone(escaper),
            task,
            others: Vec::with_capacity(2),
        }
    }

    pub(super) fn push_user_io_stats(&mut self, all: Vec<Arc<UserUpstreamTrafficStats>>) {
        for s in all {
            self.others.push(s as ArcUdpConnectTaskRemoteStats);
        }
    }

    pub(super) fn into_pair(self) -> (ArcLimitedRecvStats, ArcLimitedSendStats) {
        let s = Arc::new(self);
        (
            Arc::clone(&s) as ArcLimitedRecvStats,
            s as ArcLimitedSendStats,
        )
    }
}

impl LimitedRecvStats for DirectUdpConnectRemoteStats {
    fn add_recv_bytes(&self, size: usize) {
        let size = size as u64;
        self.escaper.udp.io.add_in_bytes(size);
        self.task.add_recv_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_recv_bytes(size));
    }

    fn add_recv_packet(&self) {
        self.escaper.udp.io.add_in_packet();
        self.task.add_recv_packet();
        self.others.iter().for_each(|stats| stats.add_recv_packet());
    }
}

impl LimitedSendStats for DirectUdpConnectRemoteStats {
    fn add_send_bytes(&self, size: usize) {
        let size = size as u64;
        self.escaper.udp.io.add_out_bytes(size);
        self.task.add_send_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_send_bytes(size));
    }

    fn add_send_packet(&self) {
        self.escaper.udp.io.add_out_packet();
        self.task.add_send_packet();
        self.others.iter().for_each(|stats| stats.add_send_packet());
    }
}
