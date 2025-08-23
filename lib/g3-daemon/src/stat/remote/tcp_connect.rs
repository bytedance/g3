/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_io_ext::{LimitedReaderStats, LimitedWriterStats};

/// task related stats used at escaper side
pub trait TcpConnectionTaskRemoteStats {
    fn add_read_bytes(&self, size: u64);
    fn add_write_bytes(&self, size: u64);
}

pub type ArcTcpConnectionTaskRemoteStats = Arc<dyn TcpConnectionTaskRemoteStats + Send + Sync>;

#[derive(Clone)]
pub struct TcpConnectionTaskRemoteStatsWrapper {
    task: ArcTcpConnectionTaskRemoteStats,
    others: Vec<ArcTcpConnectionTaskRemoteStats>,
}

impl TcpConnectionTaskRemoteStatsWrapper {
    pub fn new(task: ArcTcpConnectionTaskRemoteStats) -> Self {
        TcpConnectionTaskRemoteStatsWrapper {
            task,
            others: Vec::with_capacity(2),
        }
    }

    pub fn push_other_stats<T>(&mut self, all: Vec<Arc<T>>)
    where
        T: TcpConnectionTaskRemoteStats + Send + Sync + 'static,
    {
        for s in all {
            self.others.push(s);
        }
    }
}

impl LimitedReaderStats for TcpConnectionTaskRemoteStatsWrapper {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.add_read_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_read_bytes(size));
    }
}

impl LimitedWriterStats for TcpConnectionTaskRemoteStatsWrapper {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.add_write_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_write_bytes(size));
    }
}
