/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_io_ext::{
    ArcLimitedReaderStats, ArcLimitedWriterStats, LimitedReaderStats, LimitedWriterStats,
};

use super::{HttpForwardTaskStats, HttpRProxyServerStats};
use crate::auth::UserTrafficStats;

trait HttpForwardTaskCltStatsWrapper {
    fn add_http_read_bytes(&self, size: u64);
    fn add_http_write_bytes(&self, size: u64);
    fn add_https_read_bytes(&self, size: u64);
    fn add_https_write_bytes(&self, size: u64);
}

type ArcHttpForwardTaskCltStatsWrapper = Arc<dyn HttpForwardTaskCltStatsWrapper + Send + Sync>;

impl HttpForwardTaskCltStatsWrapper for UserTrafficStats {
    fn add_http_read_bytes(&self, size: u64) {
        self.io.http_forward.add_in_bytes(size);
    }

    fn add_http_write_bytes(&self, size: u64) {
        self.io.http_forward.add_out_bytes(size);
    }

    fn add_https_read_bytes(&self, size: u64) {
        self.io.https_forward.add_in_bytes(size);
    }

    fn add_https_write_bytes(&self, size: u64) {
        self.io.https_forward.add_out_bytes(size);
    }
}

#[derive(Clone)]
pub(crate) struct HttpForwardTaskCltWrapperStats {
    server: Arc<HttpRProxyServerStats>,
    task: Arc<HttpForwardTaskStats>,
    others: Vec<ArcHttpForwardTaskCltStatsWrapper>,
}

impl HttpForwardTaskCltWrapperStats {
    pub(crate) fn new(
        server: &Arc<HttpRProxyServerStats>,
        task: &Arc<HttpForwardTaskStats>,
    ) -> Self {
        HttpForwardTaskCltWrapperStats {
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

    pub(crate) fn split(self) -> (ArcLimitedReaderStats, ArcLimitedWriterStats) {
        let s = Arc::new(self);
        (s.clone(), s)
    }
}

impl LimitedReaderStats for HttpForwardTaskCltWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.clt.read.add_bytes(size);
        // DO NOT add server read stats, as the server stats is added in as direct stats
        self.others.iter().for_each(|s| s.add_http_read_bytes(size));
    }
}

impl LimitedWriterStats for HttpForwardTaskCltWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.clt.write.add_bytes(size);
        self.server.io_http.add_out_bytes(size);
        self.others
            .iter()
            .for_each(|s| s.add_http_write_bytes(size));
    }
}

#[derive(Clone)]
pub(crate) struct HttpsForwardTaskCltWrapperStats {
    server: Arc<HttpRProxyServerStats>,
    task: Arc<HttpForwardTaskStats>,
    others: Vec<ArcHttpForwardTaskCltStatsWrapper>,
}

impl HttpsForwardTaskCltWrapperStats {
    pub(crate) fn new(
        server: &Arc<HttpRProxyServerStats>,
        task: &Arc<HttpForwardTaskStats>,
    ) -> Self {
        HttpsForwardTaskCltWrapperStats {
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

    pub(crate) fn split(self) -> (ArcLimitedReaderStats, ArcLimitedWriterStats) {
        let s = Arc::new(self);
        (s.clone(), s)
    }
}

impl LimitedReaderStats for HttpsForwardTaskCltWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.clt.read.add_bytes(size);
        // DO NOT add server read stats, as the server stats is added in as direct stats
        self.others
            .iter()
            .for_each(|s| s.add_https_read_bytes(size));
    }
}

impl LimitedWriterStats for HttpsForwardTaskCltWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.clt.write.add_bytes(size);
        self.server.io_http.add_out_bytes(size);
        self.others
            .iter()
            .for_each(|s| s.add_https_write_bytes(size));
    }
}
