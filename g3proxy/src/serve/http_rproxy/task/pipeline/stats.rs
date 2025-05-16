/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};

use g3_io_ext::{
    ArcLimitedReaderStats, ArcLimitedWriterStats, LimitedReaderStats, LimitedWriterStats,
};

use super::HttpRProxyServerStats;

pub(crate) struct HttpRProxyPipelineStats {
    total_task: AtomicU64,
    alive_task: AtomicI32,
}

impl Default for HttpRProxyPipelineStats {
    fn default() -> Self {
        HttpRProxyPipelineStats {
            total_task: AtomicU64::new(0),
            alive_task: AtomicI32::new(0),
        }
    }
}

impl HttpRProxyPipelineStats {
    pub(super) fn add_task(&self) {
        self.total_task.fetch_add(1, Ordering::Relaxed);
        self.alive_task.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn del_task(&self) {
        self.alive_task.fetch_sub(1, Ordering::Relaxed);
    }

    pub(super) fn get_alive_task(&self) -> i32 {
        self.alive_task.load(Ordering::Relaxed)
    }
}

#[derive(Clone)]
pub(crate) struct HttpRProxyCltWrapperStats {
    server: Arc<HttpRProxyServerStats>,
}

impl HttpRProxyCltWrapperStats {
    pub(crate) fn new_for_reader(server: &Arc<HttpRProxyServerStats>) -> ArcLimitedReaderStats {
        let s = HttpRProxyCltWrapperStats {
            server: Arc::clone(server),
        };
        Arc::new(s)
    }

    pub(crate) fn new_for_writer(server: &Arc<HttpRProxyServerStats>) -> ArcLimitedWriterStats {
        let s = HttpRProxyCltWrapperStats {
            server: Arc::clone(server),
        };
        Arc::new(s)
    }
}

impl LimitedReaderStats for HttpRProxyCltWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.server.io_http.add_in_bytes(size);
    }
}

impl LimitedWriterStats for HttpRProxyCltWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.server.io_http.add_out_bytes(size);
    }
}
