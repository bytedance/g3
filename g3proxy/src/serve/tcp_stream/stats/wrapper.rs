/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_daemon::stat::task::TcpStreamTaskStats;
use g3_io_ext::{LimitedReaderStats, LimitedWriterStats};

use super::TcpStreamServerStats;
use crate::auth::UserTrafficStats;

trait TcpConnectTaskCltStatsWrapper {
    fn add_read_bytes(&self, size: u64);
    fn add_write_bytes(&self, size: u64);
}

type ArcTcpConnectTaskCltStatsWrapper = Arc<dyn TcpConnectTaskCltStatsWrapper + Send + Sync>;

impl TcpConnectTaskCltStatsWrapper for UserTrafficStats {
    fn add_read_bytes(&self, size: u64) {
        self.io.tcp_connect.add_in_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.io.tcp_connect.add_out_bytes(size);
    }
}

#[derive(Clone)]
pub(crate) struct TcpStreamTaskCltWrapperStats {
    server: Arc<TcpStreamServerStats>,
    task: Arc<TcpStreamTaskStats>,
    others: Vec<ArcTcpConnectTaskCltStatsWrapper>,
}

impl TcpStreamTaskCltWrapperStats {
    pub(crate) fn new(server: &Arc<TcpStreamServerStats>, task: &Arc<TcpStreamTaskStats>) -> Self {
        TcpStreamTaskCltWrapperStats {
            server: server.clone(),
            task: task.clone(),
            others: Vec::with_capacity(2),
        }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserTrafficStats>>) {
        for s in all {
            self.others.push(s);
        }
    }
}

impl LimitedReaderStats for TcpStreamTaskCltWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.clt.read.add_bytes(size);
        self.server.add_read(size);
        self.others.iter().for_each(|s| s.add_read_bytes(size));
    }
}

impl LimitedWriterStats for TcpStreamTaskCltWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.clt.write.add_bytes(size);
        self.server.add_write(size);
        self.others.iter().for_each(|s| s.add_write_bytes(size));
    }
}
