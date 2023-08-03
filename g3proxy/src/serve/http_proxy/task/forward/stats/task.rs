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

use crate::module::http_forward::{ArcHttpForwardTaskRemoteStats, HttpForwardTaskRemoteStats};

#[derive(Default)]
pub(crate) struct HttpForwardTaskStats {
    pub(crate) clt: TcpStreamConnectionStats,
    pub(crate) ups: TcpStreamConnectionStats,
}

impl HttpForwardTaskStats {
    #[inline]
    pub(crate) fn for_escaper(self: &Arc<Self>) -> ArcHttpForwardTaskRemoteStats {
        Arc::clone(self) as ArcHttpForwardTaskRemoteStats
    }
}

impl HttpForwardTaskRemoteStats for HttpForwardTaskStats {
    fn add_read_bytes(&self, size: u64) {
        self.ups.read.add_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.ups.write.add_bytes(size);
    }
}
