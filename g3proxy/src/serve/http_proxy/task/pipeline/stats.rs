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
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};

use g3_io_ext::{
    ArcLimitedReaderStats, ArcLimitedWriterStats, LimitedReaderStats, LimitedWriterStats,
};

use super::HttpProxyServerStats;

pub(crate) struct HttpProxyPipelineStats {
    total_task: AtomicU64,
    alive_task: AtomicI32,
}

impl Default for HttpProxyPipelineStats {
    fn default() -> Self {
        HttpProxyPipelineStats {
            total_task: AtomicU64::new(0),
            alive_task: AtomicI32::new(0),
        }
    }
}

impl HttpProxyPipelineStats {
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
pub(crate) struct HttpProxyCltWrapperStats {
    server: Arc<HttpProxyServerStats>,
}

impl HttpProxyCltWrapperStats {
    pub(crate) fn new_for_reader(server: &Arc<HttpProxyServerStats>) -> ArcLimitedReaderStats {
        let s = HttpProxyCltWrapperStats {
            server: Arc::clone(server),
        };
        Arc::new(s)
    }

    pub(crate) fn new_for_writer(server: &Arc<HttpProxyServerStats>) -> ArcLimitedWriterStats {
        let s = HttpProxyCltWrapperStats {
            server: Arc::clone(server),
        };
        Arc::new(s)
    }
}

impl LimitedReaderStats for HttpProxyCltWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.server.io_http.add_in_bytes(size);
    }
}

impl LimitedWriterStats for HttpProxyCltWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.server.io_http.add_out_bytes(size);
    }
}
