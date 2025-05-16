/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_io_ext::{LimitedReaderStats, LimitedWriterStats};

use super::{FtpOverHttpTaskStats, HttpProxyServerStats};
use crate::auth::UserTrafficStats;

trait FtpOverHttpTaskCltStatsWrapper {
    fn add_read_bytes(&self, size: u64);
    fn add_write_bytes(&self, size: u64);
}

type ArcFtpOverHttpTaskCltStatsWrapper = Arc<dyn FtpOverHttpTaskCltStatsWrapper + Send + Sync>;

impl FtpOverHttpTaskCltStatsWrapper for UserTrafficStats {
    fn add_read_bytes(&self, size: u64) {
        self.io.ftp_over_http.add_in_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.io.ftp_over_http.add_out_bytes(size);
    }
}

#[derive(Clone)]
pub(crate) struct FtpOverHttpTaskCltWrapperStats {
    server: Arc<HttpProxyServerStats>,
    task: Arc<FtpOverHttpTaskStats>,
    others: Vec<ArcFtpOverHttpTaskCltStatsWrapper>,
}

impl FtpOverHttpTaskCltWrapperStats {
    pub(crate) fn new(
        server: &Arc<HttpProxyServerStats>,
        task: &Arc<FtpOverHttpTaskStats>,
    ) -> Self {
        FtpOverHttpTaskCltWrapperStats {
            server: Arc::clone(server),
            task: Arc::clone(task),
            others: Vec::with_capacity(2),
        }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserTrafficStats>>) {
        for s in all {
            self.others.push(s);
        }
    }
}

impl LimitedReaderStats for FtpOverHttpTaskCltWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.http_client.read.add_bytes(size);
        // DO NOT add server read stats, as the server stats is added in as direct stats
        self.others.iter().for_each(|s| s.add_read_bytes(size));
    }
}

impl LimitedWriterStats for FtpOverHttpTaskCltWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.http_client.write.add_bytes(size);
        self.server.io_http.add_out_bytes(size);
        self.others.iter().for_each(|s| s.add_write_bytes(size));
    }
}
