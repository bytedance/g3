/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_io_ext::{LimitedRecvStats, LimitedSendStats};

use super::{SocksProxyServerStats, UdpAssociateTaskStats};
use crate::auth::UserTrafficStats;

trait UdpAssociateTaskCltStatsWrapper {
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

type ArcUdpAssociateTaskCltStatsWrapper = Arc<dyn UdpAssociateTaskCltStatsWrapper + Send + Sync>;

impl UdpAssociateTaskCltStatsWrapper for UserTrafficStats {
    fn add_recv_bytes(&self, size: u64) {
        self.io.socks_udp_associate.add_in_bytes(size);
    }

    fn add_recv_packets(&self, n: usize) {
        self.io.socks_udp_associate.add_in_packets(n);
    }

    fn add_send_bytes(&self, size: u64) {
        self.io.socks_udp_associate.add_out_bytes(size);
    }

    fn add_send_packets(&self, n: usize) {
        self.io.socks_udp_associate.add_out_packets(n);
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
            self.others.push(s);
        }
    }
}

impl LimitedRecvStats for UdpAssociateTaskCltWrapperStats {
    fn add_recv_bytes(&self, size: usize) {
        let size = size as u64;
        self.server.io_udp.add_in_bytes(size);
        self.task.clt.recv.add_bytes(size);
        self.others.iter().for_each(|s| s.add_recv_bytes(size));
    }

    fn add_recv_packets(&self, n: usize) {
        self.server.io_udp.add_in_packets(n);
        self.task.clt.recv.add_packets(n);
        self.others.iter().for_each(|s| s.add_recv_packets(n));
    }
}

impl LimitedSendStats for UdpAssociateTaskCltWrapperStats {
    fn add_send_bytes(&self, size: usize) {
        let size = size as u64;
        self.server.io_udp.add_out_bytes(size);
        self.task.clt.send.add_bytes(size);
        self.others.iter().for_each(|s| s.add_send_bytes(size));
    }

    fn add_send_packets(&self, n: usize) {
        self.server.io_udp.add_out_packets(n);
        self.task.clt.send.add_packets(n);
        self.others.iter().for_each(|s| s.add_send_packets(n));
    }
}
