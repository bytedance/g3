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

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct LogSnapshot {
    pub io: LogIoSnapshot,
    pub drop: LogDropSnapshot,
}

#[derive(Default, Debug, Eq, PartialEq)]
pub struct LogIoSnapshot {
    pub total: u64,
    pub passed: u64,
    pub size: u64,
}

#[derive(Default, Debug, Eq, PartialEq)]
pub struct LogDropSnapshot {
    pub format_failed: u64,
    pub channel_closed: u64,
    pub channel_overflow: u64,
    pub peer_unreachable: u64,
}

#[derive(Default)]
pub struct LogStats {
    pub io: LogIoStats,
    pub drop: LogDropStats,
}

impl LogStats {
    pub fn snapshot(&self) -> LogSnapshot {
        LogSnapshot {
            io: self.io.snapshot(),
            drop: self.drop.snapshot(),
        }
    }
}

#[derive(Default)]
pub struct LogIoStats {
    total: AtomicU64,
    passed: AtomicU64,
    size: AtomicU64,
}

impl LogIoStats {
    pub fn snapshot(&self) -> LogIoSnapshot {
        LogIoSnapshot {
            total: self.total.load(Ordering::Relaxed),
            passed: self.passed.load(Ordering::Relaxed),
            size: self.size.load(Ordering::Relaxed),
        }
    }

    pub fn add_total(&self) {
        self.total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_passed(&self) {
        self.passed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_size(&self, size: usize) {
        self.size.fetch_add(size as u64, Ordering::Relaxed);
    }
}

#[derive(Default)]
pub struct LogDropStats {
    format_failed: AtomicU64,
    channel_closed: AtomicU64,
    channel_overflow: AtomicU64,
    peer_unreachable: AtomicU64,
}

impl LogDropStats {
    pub fn snapshot(&self) -> LogDropSnapshot {
        LogDropSnapshot {
            format_failed: self.format_failed.load(Ordering::Relaxed),
            channel_closed: self.channel_closed.load(Ordering::Relaxed),
            channel_overflow: self.channel_overflow.load(Ordering::Relaxed),
            peer_unreachable: self.peer_unreachable.load(Ordering::Relaxed),
        }
    }

    pub fn add_format_failed(&self) {
        self.format_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_channel_closed(&self) {
        self.channel_closed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_channel_overflow(&self) {
        self.channel_overflow.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_peer_unreachable(&self) {
        self.peer_unreachable.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_drop_stats() {
        let stats = LogDropStats::default();
        stats.add_format_failed();
        stats.add_channel_closed();
        stats.add_channel_overflow();
        stats.add_peer_unreachable();
        assert_eq!(
            stats.snapshot(),
            LogDropSnapshot {
                format_failed: 1,
                channel_closed: 1,
                channel_overflow: 1,
                peer_unreachable: 1
            }
        )
    }

    #[test]
    fn t_io_stats() {
        let stats = LogIoStats::default();
        stats.add_total();
        stats.add_passed();
        stats.add_size(1024);
        assert_eq!(
            stats.snapshot(),
            LogIoSnapshot {
                total: 1,
                passed: 1,
                size: 1024
            }
        )
    }
}
