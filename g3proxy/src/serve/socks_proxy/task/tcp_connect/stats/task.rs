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

use g3_daemon::stat::remote::{ArcTcpConnectionTaskRemoteStats, TcpConnectionTaskRemoteStats};

pub(crate) struct TcpConnectHalfConnectionStats {
    bytes: u64,
}

impl TcpConnectHalfConnectionStats {
    fn new() -> Self {
        TcpConnectHalfConnectionStats { bytes: 0 }
    }

    pub(crate) fn get_bytes(&self) -> u64 {
        self.bytes
    }

    pub(crate) fn add_bytes(&self, size: u64) {
        unsafe {
            let r = &self.bytes as *const u64 as *mut u64;
            *r += size;
        }
    }
}

pub(crate) struct TcpConnectConnectionStats {
    pub(crate) read: TcpConnectHalfConnectionStats,
    pub(crate) write: TcpConnectHalfConnectionStats,
}

impl TcpConnectConnectionStats {
    fn new() -> Self {
        TcpConnectConnectionStats {
            read: TcpConnectHalfConnectionStats::new(),
            write: TcpConnectHalfConnectionStats::new(),
        }
    }
}

pub(crate) struct TcpConnectTaskStats {
    pub(crate) clt: TcpConnectConnectionStats,
    pub(crate) ups: TcpConnectConnectionStats,
}

impl TcpConnectTaskStats {
    pub(crate) fn new() -> Self {
        TcpConnectTaskStats {
            clt: TcpConnectConnectionStats::new(),
            ups: TcpConnectConnectionStats::new(),
        }
    }

    #[inline]
    pub(crate) fn for_escaper(self: &Arc<Self>) -> ArcTcpConnectionTaskRemoteStats {
        Arc::clone(self) as ArcTcpConnectionTaskRemoteStats
    }
}

impl TcpConnectionTaskRemoteStats for TcpConnectTaskStats {
    fn add_read_bytes(&self, size: u64) {
        self.ups.read.add_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.ups.write.add_bytes(size);
    }
}
