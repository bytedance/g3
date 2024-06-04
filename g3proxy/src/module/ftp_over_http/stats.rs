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
pub(crate) struct FtpControlRemoteWrapperStats<T> {
    escaper: Arc<T>,
    task: ArcFtpTaskRemoteControlStats,
    others: Vec<ArcFtpTaskRemoteControlStats>,
}

impl<T: FtpTaskRemoteControlStats> FtpControlRemoteWrapperStats<T> {
    pub(crate) fn new(escaper: &Arc<T>, task: ArcFtpTaskRemoteControlStats) -> Self {
        FtpControlRemoteWrapperStats {
            escaper: Arc::clone(escaper),
            task,
            others: Vec::with_capacity(2),
        }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserUpstreamTrafficStats>>) {
        for s in all {
            self.others.push(s);
        }
    }
}

impl<T: FtpTaskRemoteControlStats> LimitedReaderStats for FtpControlRemoteWrapperStats<T> {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.escaper.add_read_bytes(size);
        self.task.add_read_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_read_bytes(size));
    }
}

impl<T: FtpTaskRemoteControlStats> LimitedWriterStats for FtpControlRemoteWrapperStats<T> {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.escaper.add_write_bytes(size);
        self.task.add_write_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_write_bytes(size));
    }
}

#[derive(Clone)]
pub(crate) struct FtpTransferRemoteWrapperStats<T> {
    escaper: Arc<T>,
    task: ArcFtpTaskRemoteTransferStats,
    others: Vec<ArcFtpTaskRemoteTransferStats>,
}

impl<T: FtpTaskRemoteTransferStats> FtpTransferRemoteWrapperStats<T> {
    pub(crate) fn new(escaper: &Arc<T>, task: ArcFtpTaskRemoteTransferStats) -> Self {
        FtpTransferRemoteWrapperStats {
            escaper: Arc::clone(escaper),
            task,
            others: Vec::with_capacity(2),
        }
    }

    pub(crate) fn push_user_io_stats(&mut self, all: Vec<Arc<UserUpstreamTrafficStats>>) {
        for s in all {
            self.others.push(s);
        }
    }
}

impl<T: FtpTaskRemoteTransferStats> LimitedReaderStats for FtpTransferRemoteWrapperStats<T> {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.escaper.add_read_bytes(size);
        self.task.add_read_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_read_bytes(size));
    }
}

impl<T: FtpTaskRemoteTransferStats> LimitedWriterStats for FtpTransferRemoteWrapperStats<T> {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.escaper.add_write_bytes(size);
        self.task.add_write_bytes(size);
        self.others
            .iter()
            .for_each(|stats| stats.add_write_bytes(size));
    }
}
