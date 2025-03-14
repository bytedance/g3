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

use g3_io_ext::{LimitedReaderStats, LimitedWriterStats};

use crate::auth::UserUpstreamTrafficStats;

/// task related stats used at escaper side
pub(crate) trait FtpTaskRemoteControlStats {
    fn add_read_bytes(&self, size: u64);
    fn add_write_bytes(&self, size: u64);
}

pub(crate) type ArcFtpTaskRemoteControlStats = Arc<dyn FtpTaskRemoteControlStats + Send + Sync>;

impl FtpTaskRemoteControlStats for UserUpstreamTrafficStats {
    fn add_read_bytes(&self, size: u64) {
        self.io.tcp.add_in_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.io.tcp.add_out_bytes(size);
    }
}

/// task related stats used at escaper side
pub(crate) trait FtpTaskRemoteTransferStats {
    fn add_read_bytes(&self, size: u64);
    fn add_write_bytes(&self, size: u64);
}

pub(crate) type ArcFtpTaskRemoteTransferStats = Arc<dyn FtpTaskRemoteTransferStats + Send + Sync>;

impl FtpTaskRemoteTransferStats for UserUpstreamTrafficStats {
    fn add_read_bytes(&self, size: u64) {
        self.io.tcp.add_in_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.io.tcp.add_in_bytes(size);
    }
}

#[derive(Clone)]
pub(crate) struct FtpControlRemoteWrapperStats {
    all: Vec<ArcFtpTaskRemoteControlStats>,
}

impl FtpControlRemoteWrapperStats {
    pub(crate) fn new(
        escaper: ArcFtpTaskRemoteControlStats,
        task: ArcFtpTaskRemoteControlStats,
    ) -> Self {
        let mut all = Vec::with_capacity(4);
        all.push(task);
        all.push(escaper);
        FtpControlRemoteWrapperStats { all }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserUpstreamTrafficStats>>) {
        for s in all {
            self.all.push(s);
        }
    }
}

impl LimitedReaderStats for FtpControlRemoteWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.all.iter().for_each(|stats| stats.add_read_bytes(size));
    }
}

impl LimitedWriterStats for FtpControlRemoteWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.all
            .iter()
            .for_each(|stats| stats.add_write_bytes(size));
    }
}

#[derive(Clone)]
pub(crate) struct FtpTransferRemoteWrapperStats {
    all: Vec<ArcFtpTaskRemoteTransferStats>,
}

impl FtpTransferRemoteWrapperStats {
    pub(crate) fn new(
        escaper: ArcFtpTaskRemoteTransferStats,
        task: ArcFtpTaskRemoteTransferStats,
    ) -> Self {
        let mut all = Vec::with_capacity(4);
        all.push(task);
        all.push(escaper);
        FtpTransferRemoteWrapperStats { all }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserUpstreamTrafficStats>>) {
        for s in all {
            self.all.push(s);
        }
    }
}

impl LimitedReaderStats for FtpTransferRemoteWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.all.iter().for_each(|stats| stats.add_read_bytes(size));
    }
}

impl LimitedWriterStats for FtpTransferRemoteWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.all
            .iter()
            .for_each(|stats| stats.add_write_bytes(size));
    }
}
