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
use g3_io_ext::{LimitedReaderStats, LimitedWriterStats};

use crate::auth::UserUpstreamTrafficStats;

#[derive(Clone)]
pub(crate) struct TcpConnectRemoteWrapperStats {
    all: Vec<ArcTcpConnectionTaskRemoteStats>,
}

impl TcpConnectRemoteWrapperStats {
    pub(crate) fn new(
        escaper: ArcTcpConnectionTaskRemoteStats,
        task: ArcTcpConnectionTaskRemoteStats,
    ) -> Self {
        let mut all = Vec::with_capacity(4);
        all.push(task);
        all.push(escaper);
        TcpConnectRemoteWrapperStats { all }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserUpstreamTrafficStats>>) {
        for s in all {
            self.all.push(s);
        }
    }
}

impl LimitedReaderStats for TcpConnectRemoteWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.all.iter().for_each(|stats| stats.add_read_bytes(size));
    }
}

impl LimitedWriterStats for TcpConnectRemoteWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.all
            .iter()
            .for_each(|stats| stats.add_write_bytes(size));
    }
}
