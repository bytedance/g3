/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

pub trait LimitedRecvStats {
    fn add_recv_bytes(&self, size: usize);
    fn add_recv_packet(&self) {
        self.add_recv_packets(1);
    }
    fn add_recv_packets(&self, n: usize);
}
pub type ArcLimitedRecvStats = Arc<dyn LimitedRecvStats + Send + Sync>;

pub trait LimitedSendStats {
    fn add_send_bytes(&self, size: usize);
    fn add_send_packet(&self) {
        self.add_send_packets(1);
    }
    fn add_send_packets(&self, n: usize);
}
pub type ArcLimitedSendStats = Arc<dyn LimitedSendStats + Send + Sync>;
