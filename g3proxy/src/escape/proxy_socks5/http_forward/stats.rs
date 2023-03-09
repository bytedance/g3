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

use g3_io_ext::{ArcLimitedWriterStats, LimitedWriterStats};

use super::ProxySocks5EscaperStats;
use crate::auth::UserUpstreamTrafficStats;
use crate::module::http_forward::ArcHttpForwardTaskRemoteStats;

#[derive(Clone)]
pub(super) struct ProxySocks5MixedRemoteStats {
    escaper: Arc<ProxySocks5EscaperStats>,
    task: ArcHttpForwardTaskRemoteStats,
    others: Vec<ArcHttpForwardTaskRemoteStats>,
}

impl ProxySocks5MixedRemoteStats {
    pub(super) fn new(
        escaper: &Arc<ProxySocks5EscaperStats>,
        task: &ArcHttpForwardTaskRemoteStats,
    ) -> Self {
        ProxySocks5MixedRemoteStats {
            escaper: Arc::clone(escaper),
            task: Arc::clone(task),
            others: Vec::with_capacity(2),
        }
    }

    pub(super) fn push_user_io_stats_by_ref(&mut self, all: &[Arc<UserUpstreamTrafficStats>]) {
        for s in all {
            self.others.push(s.clone() as ArcHttpForwardTaskRemoteStats);
        }
    }

    pub(super) fn push_user_io_stats(&mut self, all: Vec<Arc<UserUpstreamTrafficStats>>) {
        for s in all {
            self.others.push(s.clone() as ArcHttpForwardTaskRemoteStats);
        }
    }

    pub(super) fn into_writer(self) -> ArcLimitedWriterStats {
        Arc::new(self)
    }
}

impl LimitedWriterStats for ProxySocks5MixedRemoteStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.escaper.tcp.io.add_out_bytes(size);
        self.task.add_write_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_write_bytes(size));
    }
}
