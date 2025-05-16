/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_daemon::stat::task::{TcpStreamConnectionStats, TcpStreamTaskStats};
use g3_io_ext::{LimitedReaderStats, LimitedWriterStats};

use crate::module::stream::StreamServerStats;

#[derive(Clone)]
pub(crate) struct StreamAcceptTaskCltWrapperStats {
    server: Arc<StreamServerStats>,
    conn: Arc<TcpStreamConnectionStats>,
}

impl StreamAcceptTaskCltWrapperStats {
    pub(crate) fn new(
        server: &Arc<StreamServerStats>,
        conn: &Arc<TcpStreamConnectionStats>,
    ) -> Self {
        StreamAcceptTaskCltWrapperStats {
            server: Arc::clone(server),
            conn: Arc::clone(conn),
        }
    }
}

impl LimitedReaderStats for StreamAcceptTaskCltWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.conn.read.add_bytes(size);
        self.server.add_read(size);
    }
}

impl LimitedWriterStats for StreamAcceptTaskCltWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.conn.write.add_bytes(size);
        self.server.add_write(size);
    }
}

#[derive(Clone)]
pub(crate) struct StreamRelayTaskCltWrapperStats {
    server: Arc<StreamServerStats>,
    task: Arc<TcpStreamTaskStats>,
}

impl StreamRelayTaskCltWrapperStats {
    pub(crate) fn new(server: &Arc<StreamServerStats>, task: &Arc<TcpStreamTaskStats>) -> Self {
        StreamRelayTaskCltWrapperStats {
            server: Arc::clone(server),
            task: Arc::clone(task),
        }
    }
}

impl LimitedReaderStats for StreamRelayTaskCltWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.clt.read.add_bytes(size);
        self.server.add_read(size);
    }
}

impl LimitedWriterStats for StreamRelayTaskCltWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.clt.write.add_bytes(size);
        self.server.add_write(size);
    }
}
