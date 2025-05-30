/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_io_ext::{LimitedReaderStats, LimitedWriterStats};

use crate::auth::UserUpstreamTrafficStats;

/// task related stats used at escaper side
pub(crate) trait HttpForwardTaskRemoteStats {
    fn add_read_bytes(&self, size: u64);
    fn add_write_bytes(&self, size: u64);
}

pub(crate) type ArcHttpForwardTaskRemoteStats = Arc<dyn HttpForwardTaskRemoteStats + Send + Sync>;

impl HttpForwardTaskRemoteStats for UserUpstreamTrafficStats {
    fn add_read_bytes(&self, size: u64) {
        self.io.tcp.add_in_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.io.tcp.add_out_bytes(size);
    }
}

#[derive(Clone)]
pub(crate) struct HttpForwardTaskRemoteWrapperStats {
    all: Vec<ArcHttpForwardTaskRemoteStats>,
}

impl HttpForwardTaskRemoteWrapperStats {
    pub(crate) fn new(task: ArcHttpForwardTaskRemoteStats) -> Self {
        let mut all = Vec::with_capacity(3);
        all.push(task);
        HttpForwardTaskRemoteWrapperStats { all }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserUpstreamTrafficStats>>) {
        for s in all {
            self.all.push(s as ArcHttpForwardTaskRemoteStats);
        }
    }
}

impl LimitedReaderStats for HttpForwardTaskRemoteWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.all.iter().for_each(|stats| stats.add_read_bytes(size));
    }
}

impl LimitedWriterStats for HttpForwardTaskRemoteWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.all
            .iter()
            .for_each(|stats| stats.add_write_bytes(size));
    }
}

#[derive(Clone)]
pub(crate) struct HttpForwardRemoteWrapperStats {
    all: Vec<ArcHttpForwardTaskRemoteStats>,
}

impl HttpForwardRemoteWrapperStats {
    pub(crate) fn new(
        escaper: ArcHttpForwardTaskRemoteStats,
        task: &ArcHttpForwardTaskRemoteStats,
    ) -> Self {
        let mut all = Vec::with_capacity(4);
        all.push(task.clone());
        all.push(escaper);
        HttpForwardRemoteWrapperStats { all }
    }

    pub(crate) fn push_user_io_stats_by_ref(&mut self, all: &[Arc<UserUpstreamTrafficStats>]) {
        for s in all {
            self.all.push(s.clone());
        }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserUpstreamTrafficStats>>) {
        for s in all {
            self.all.push(s);
        }
    }
}

impl LimitedWriterStats for HttpForwardRemoteWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.all
            .iter()
            .for_each(|stats| stats.add_write_bytes(size));
    }
}
