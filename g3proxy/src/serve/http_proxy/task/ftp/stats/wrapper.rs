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
            self.others.push(s as ArcFtpOverHttpTaskCltStatsWrapper);
        }
    }

    pub(crate) fn split(self) -> (ArcLimitedReaderStats, ArcLimitedWriterStats) {
        let s = Arc::new(self);
        (
            Arc::clone(&s) as ArcLimitedReaderStats,
            s as ArcLimitedWriterStats,
        )
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
