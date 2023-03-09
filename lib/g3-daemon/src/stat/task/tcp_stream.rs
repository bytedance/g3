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

use crate::stat::remote::{ArcTcpConnectionTaskRemoteStats, TcpConnectionTaskRemoteStats};

#[derive(Copy, Clone, Default)]
pub struct TcpStreamHalfConnectionStats {
    bytes: u64,
}

impl TcpStreamHalfConnectionStats {
    pub fn get_bytes(&self) -> u64 {
        self.bytes
    }

    pub fn add_bytes(&self, size: u64) {
        unsafe {
            let r = &self.bytes as *const u64 as *mut u64;
            *r += size;
        }
    }
}

#[derive(Copy, Clone, Default)]
pub struct TcpStreamConnectionStats {
    pub read: TcpStreamHalfConnectionStats,
    pub write: TcpStreamHalfConnectionStats,
}

#[derive(Default)]
pub struct TcpStreamTaskStats {
    pub clt: TcpStreamConnectionStats,
    pub ups: TcpStreamConnectionStats,
}

impl TcpStreamTaskStats {
    pub fn with_clt_stats(clt: TcpStreamConnectionStats) -> Self {
        TcpStreamTaskStats {
            clt,
            ups: TcpStreamConnectionStats::default(),
        }
    }

    #[inline]
    pub fn for_escaper(self: &Arc<Self>) -> ArcTcpConnectionTaskRemoteStats {
        Arc::clone(self) as ArcTcpConnectionTaskRemoteStats
    }
}

impl TcpConnectionTaskRemoteStats for TcpStreamTaskStats {
    fn add_read_bytes(&self, size: u64) {
        self.ups.read.add_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.ups.write.add_bytes(size);
    }
}
