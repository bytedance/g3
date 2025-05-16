/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_io_ext::{
    ArcLimitedReaderStats, ArcLimitedWriterStats, LimitedReaderStats, LimitedWriterStats,
};

use super::SocksProxyServerStats;

#[derive(Clone)]
pub(crate) struct SocksProxyCltWrapperStats {
    server: Arc<SocksProxyServerStats>,
}

impl SocksProxyCltWrapperStats {
    pub(crate) fn new_pair(
        server: &Arc<SocksProxyServerStats>,
    ) -> (ArcLimitedReaderStats, ArcLimitedWriterStats) {
        let s = SocksProxyCltWrapperStats {
            server: Arc::clone(server),
        };
        // Clone is OK as we only have smart pointer in s
        (Arc::new(s.clone()), Arc::new(s))
    }
}

impl LimitedReaderStats for SocksProxyCltWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.server.io_tcp.add_in_bytes(size);
    }
}

impl LimitedWriterStats for SocksProxyCltWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.server.io_tcp.add_out_bytes(size);
    }
}
