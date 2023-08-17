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
    task: ArcHttpForwardTaskRemoteStats,
    others: Vec<ArcHttpForwardTaskRemoteStats>,
}

impl HttpForwardTaskRemoteWrapperStats {
    pub(crate) fn new(task: ArcHttpForwardTaskRemoteStats) -> Self {
        HttpForwardTaskRemoteWrapperStats {
            task,
            others: Vec::with_capacity(2),
        }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserUpstreamTrafficStats>>) {
        for s in all {
            self.others.push(s as ArcHttpForwardTaskRemoteStats);
        }
    }
}

impl LimitedReaderStats for HttpForwardTaskRemoteWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.add_read_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_read_bytes(size));
    }
}

impl LimitedWriterStats for HttpForwardTaskRemoteWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.add_write_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_write_bytes(size));
    }
}

#[derive(Clone)]
pub(crate) struct HttpForwardRemoteWrapperStats<T> {
    escaper: Arc<T>,
    task: ArcHttpForwardTaskRemoteStats,
    others: Vec<ArcHttpForwardTaskRemoteStats>,
}

impl<T: HttpForwardTaskRemoteStats> HttpForwardRemoteWrapperStats<T> {
    pub(crate) fn new(escaper: &Arc<T>, task: &ArcHttpForwardTaskRemoteStats) -> Self {
        HttpForwardRemoteWrapperStats {
            escaper: Arc::clone(escaper),
            task: Arc::clone(task),
            others: Vec::with_capacity(2),
        }
    }

    pub(crate) fn push_user_io_stats_by_ref(&mut self, all: &[Arc<UserUpstreamTrafficStats>]) {
        for s in all {
            self.others.push(s.clone() as _);
        }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserUpstreamTrafficStats>>) {
        for s in all {
            self.others.push(s as _);
        }
    }
}

impl<T: HttpForwardTaskRemoteStats> LimitedWriterStats for HttpForwardRemoteWrapperStats<T> {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.escaper.add_write_bytes(size);
        self.task.add_write_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_write_bytes(size));
    }
}
