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

use g3_io_ext::{
    ArcLimitedReaderStats, ArcLimitedWriterStats, LimitedReaderStats, LimitedWriterStats,
};

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
            self.others.push(s as ArcTcpConnectionTaskRemoteStats);
        }
    }

    pub fn into_pair(self) -> (ArcLimitedReaderStats, ArcLimitedWriterStats) {
        let s = Arc::new(self);
        (
            Arc::clone(&s) as ArcLimitedReaderStats,
            s as ArcLimitedWriterStats,
        )
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
