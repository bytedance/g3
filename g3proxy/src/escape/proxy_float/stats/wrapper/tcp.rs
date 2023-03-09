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

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_io_ext::{
    ArcLimitedReaderStats, ArcLimitedWriterStats, LimitedReaderStats, LimitedWriterStats,
};

use super::ProxyFloatEscaperStats;
use crate::auth::UserUpstreamTrafficStats;

#[derive(Clone)]
pub(crate) struct ProxyTcpRemoteStats {
    escaper: Arc<ProxyFloatEscaperStats>,
    task: ArcTcpConnectionTaskRemoteStats,
    others: Vec<ArcTcpConnectionTaskRemoteStats>,
}

impl ProxyTcpRemoteStats {
    pub(crate) fn new(
        escaper: &Arc<ProxyFloatEscaperStats>,
        task: ArcTcpConnectionTaskRemoteStats,
    ) -> Self {
        ProxyTcpRemoteStats {
            escaper: Arc::clone(escaper),
            task,
            others: Vec::with_capacity(2),
        }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserUpstreamTrafficStats>>) {
        for s in all {
            self.others.push(s as ArcTcpConnectionTaskRemoteStats);
        }
    }

    pub(crate) fn into_pair(self) -> (ArcLimitedReaderStats, ArcLimitedWriterStats) {
        let s = Arc::new(self);
        (
            Arc::clone(&s) as ArcLimitedReaderStats,
            s as ArcLimitedWriterStats,
        )
    }
}

impl LimitedReaderStats for ProxyTcpRemoteStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.escaper.tcp.io.add_in_bytes(size);
        self.task.add_read_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_read_bytes(size));
    }
}

impl LimitedWriterStats for ProxyTcpRemoteStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.escaper.tcp.io.add_out_bytes(size);
        self.task.add_write_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_write_bytes(size));
    }
}
