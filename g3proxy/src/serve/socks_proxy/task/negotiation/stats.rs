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
