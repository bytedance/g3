/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_daemon::stat::task::TcpStreamTaskStats;
use g3_io_ext::{
    ArcLimitedReaderStats, ArcLimitedWriterStats, LimitedReaderStats, LimitedWriterStats,
};

use super::TcpStreamServerStats;

#[derive(Clone)]
pub(crate) struct TcpStreamTaskCltWrapperStats {
    server: Arc<TcpStreamServerStats>,
    task: Arc<TcpStreamTaskStats>,
}

impl TcpStreamTaskCltWrapperStats {
    pub(crate) fn new_pair(
        server: &Arc<TcpStreamServerStats>,
        task: &Arc<TcpStreamTaskStats>,
    ) -> (ArcLimitedReaderStats, ArcLimitedWriterStats) {
        let s = TcpStreamTaskCltWrapperStats {
            server: Arc::clone(server),
            task: Arc::clone(task),
        };
        // Clone is OK as we only have smart pointer in s
        (Arc::new(s.clone()), Arc::new(s))
    }
}

impl LimitedReaderStats for TcpStreamTaskCltWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.clt.read.add_bytes(size);
        self.server.add_read(size);
    }
}

impl LimitedWriterStats for TcpStreamTaskCltWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.clt.write.add_bytes(size);
        self.server.add_write(size);
    }
}
