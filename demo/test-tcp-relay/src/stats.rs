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

use std::cell::UnsafeCell;
use std::sync::Arc;

use g3_io_ext::{
    ArcLimitedReaderStats, ArcLimitedWriterStats, LimitedReaderStats, LimitedWriterStats,
};

#[derive(Debug, Default)]
struct HalfConnectionStats {
    bytes: UnsafeCell<u64>,
}

unsafe impl Sync for HalfConnectionStats {}

impl HalfConnectionStats {
    fn add_bytes(&self, size: u64) {
        let r = unsafe { &mut *self.bytes.get() };
        *r += size;
    }
}

#[derive(Debug, Default)]
struct ConnectionStats {
    read: HalfConnectionStats,
    write: HalfConnectionStats,
}

#[derive(Debug, Default)]
pub struct TaskStats {
    clt: ConnectionStats,
    ups: ConnectionStats,
}

impl TaskStats {
    fn print(&self) {
        println!("{self:?}");
    }
}

impl Drop for TaskStats {
    fn drop(&mut self) {
        self.print()
    }
}

#[derive(Clone)]
pub struct CltStats {
    task: Arc<TaskStats>,
}

impl CltStats {
    pub fn new_pair(task: Arc<TaskStats>) -> (ArcLimitedReaderStats, ArcLimitedWriterStats) {
        let s = CltStats { task };
        (Arc::new(s.clone()), Arc::new(s))
    }
}

impl LimitedReaderStats for CltStats {
    fn add_read_bytes(&self, size: usize) {
        self.task.clt.read.add_bytes(size as u64);
    }
}

impl LimitedWriterStats for CltStats {
    fn add_write_bytes(&self, size: usize) {
        self.task.clt.write.add_bytes(size as u64);
    }
}

#[derive(Clone)]
pub struct UpsStats {
    task: Arc<TaskStats>,
}

impl UpsStats {
    pub fn new_pair(task: Arc<TaskStats>) -> (ArcLimitedReaderStats, ArcLimitedWriterStats) {
        let s = UpsStats { task };
        (Arc::new(s.clone()), Arc::new(s))
    }
}

impl LimitedReaderStats for UpsStats {
    fn add_read_bytes(&self, size: usize) {
        self.task.ups.read.add_bytes(size as u64);
    }
}

impl LimitedWriterStats for UpsStats {
    fn add_write_bytes(&self, size: usize) {
        self.task.ups.write.add_bytes(size as u64);
    }
}
