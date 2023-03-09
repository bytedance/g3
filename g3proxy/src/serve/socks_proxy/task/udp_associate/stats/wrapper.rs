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

use super::{SocksProxyServerStats, UdpAssociateTaskStats};
use crate::auth::UserTrafficStats;

trait UdpAssociateTaskCltStatsWrapper {
    fn add_recv_bytes(&self, size: u64);
    fn add_recv_packet(&self);
    fn add_send_bytes(&self, size: u64);
    fn add_send_packet(&self);
}

type ArcUdpAssociateTaskCltStatsWrapper = Arc<dyn UdpAssociateTaskCltStatsWrapper + Send + Sync>;

impl UdpAssociateTaskCltStatsWrapper for UserTrafficStats {
    fn add_recv_bytes(&self, size: u64) {
        self.io.socks_udp_associate.add_in_bytes(size);
    }

    fn add_recv_packet(&self) {
        self.io.socks_udp_associate.add_in_packet();
    }

    fn add_send_bytes(&self, size: u64) {
        self.io.socks_udp_associate.add_out_bytes(size);
    }

    fn add_send_packet(&self) {
        self.io.socks_udp_associate.add_out_packet();
    }
}

#[derive(Clone)]
pub(crate) struct UdpAssociateTaskCltWrapperStats {
    server: Arc<SocksProxyServerStats>,
    task: Arc<UdpAssociateTaskStats>,
    others: Vec<ArcUdpAssociateTaskCltStatsWrapper>,
}

impl UdpAssociateTaskCltWrapperStats {
    pub(crate) fn new(
        server: &Arc<SocksProxyServerStats>,
        task: &Arc<UdpAssociateTaskStats>,
    ) -> Self {
        UdpAssociateTaskCltWrapperStats {
            server: Arc::clone(server),
            task: Arc::clone(task),
            others: Vec::with_capacity(2),
        }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserTrafficStats>>) {
        for s in all {
            self.others.push(s as ArcUdpAssociateTaskCltStatsWrapper);
        }
    }

    pub(crate) fn split(self) -> (ArcLimitedRecvStats, ArcLimitedSendStats) {
        let s = Arc::new(self);
        (
            Arc::clone(&s) as ArcLimitedRecvStats,
            s as ArcLimitedSendStats,
        )
    }
}

impl LimitedRecvStats for UdpAssociateTaskCltWrapperStats {
    fn add_recv_bytes(&self, size: usize) {
        let size = size as u64;
        self.server.io_udp.add_in_bytes(size);
        self.task.clt.recv.add_bytes(size);
        self.others.iter().for_each(|s| s.add_recv_bytes(size));
    }

    fn add_recv_packet(&self) {
        self.server.io_udp.add_in_packet();
        self.task.clt.recv.add_packet();
        self.others.iter().for_each(|s| s.add_recv_packet());
    }
}

impl LimitedSendStats for UdpAssociateTaskCltWrapperStats {
    fn add_send_bytes(&self, size: usize) {
        let size = size as u64;
        self.server.io_udp.add_out_bytes(size);
        self.task.clt.send.add_bytes(size);
        self.others.iter().for_each(|s| s.add_send_bytes(size));
    }

    fn add_send_packet(&self) {
        self.server.io_udp.add_out_packet();
        self.task.clt.send.add_packet();
        self.others.iter().for_each(|s| s.add_send_packet());
    }
}
