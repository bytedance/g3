/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_daemon::stat::task::TcpStreamConnectionStats;

use crate::module::http_forward::HttpForwardTaskRemoteStats;

#[derive(Default)]
pub(crate) struct HttpForwardTaskStats {
    pub(crate) clt: TcpStreamConnectionStats,
    pub(crate) ups: TcpStreamConnectionStats,
}

impl HttpForwardTaskRemoteStats for HttpForwardTaskStats {
    fn add_read_bytes(&self, size: u64) {
        self.ups.read.add_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.ups.write.add_bytes(size);
    }
}
