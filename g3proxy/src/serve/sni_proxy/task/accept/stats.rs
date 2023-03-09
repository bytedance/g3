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

use g3_daemon::stat::task::TcpStreamConnectionStats;
use g3_io_ext::{
    ArcLimitedReaderStats, ArcLimitedWriterStats, LimitedReaderStats, LimitedWriterStats,
};

use crate::serve::tcp_stream::TcpStreamServerStats;

#[derive(Clone)]
pub(super) struct SniProxyCltWrapperStats {
    server: Arc<TcpStreamServerStats>,
    conn: Arc<TcpStreamConnectionStats>,
}

impl SniProxyCltWrapperStats {
    pub(super) fn new_pair(
        server: &Arc<TcpStreamServerStats>,
        conn: &Arc<TcpStreamConnectionStats>,
    ) -> (ArcLimitedReaderStats, ArcLimitedWriterStats) {
        let s = SniProxyCltWrapperStats {
            server: Arc::clone(server),
            conn: Arc::clone(conn),
        };
        // Clone is OK as we only have smart pointer in s
        (Arc::new(s.clone()), Arc::new(s))
    }
}

impl LimitedReaderStats for SniProxyCltWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.conn.read.add_bytes(size);
        self.server.add_read(size);
    }
}

impl LimitedWriterStats for SniProxyCltWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.conn.write.add_bytes(size);
        self.server.add_write(size);
    }
}
