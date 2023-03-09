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

use std::sync::atomic::{AtomicIsize, AtomicU64, Ordering};

use g3_types::stats::StatId;

#[derive(Default)]
pub struct ListenSnapshot {
    pub accepted: u64,
    pub dropped: u64,
    pub timeout: u64,
    pub failed: u64,
}

#[derive(Debug)]
pub struct ListenStats {
    name: String,
    id: StatId,

    runtime_count: AtomicIsize,
    accepted: AtomicU64,
    dropped: AtomicU64,
    timeout: AtomicU64,
    failed: AtomicU64,
}

impl ListenStats {
    pub fn new(name: &str) -> Self {
        ListenStats {
            name: name.to_string(),
            id: StatId::new(),
            runtime_count: AtomicIsize::new(0),
            accepted: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            timeout: AtomicU64::new(0),
            failed: AtomicU64::new(0),
        }
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn stat_id(&self) -> StatId {
        self.id
    }

    pub fn add_running_runtime(&self) {
        self.runtime_count.fetch_add(1, Ordering::Relaxed);
    }
    pub fn del_running_runtime(&self) {
        self.runtime_count.fetch_sub(1, Ordering::Relaxed);
    }
    pub fn get_running_runtime_count(&self) -> isize {
        self.runtime_count.load(Ordering::Relaxed)
    }
    #[inline]
    pub fn is_running(&self) -> bool {
        self.get_running_runtime_count() > 0
    }

    pub fn add_accepted(&self) {
        self.accepted.fetch_add(1, Ordering::Relaxed);
    }
    pub fn get_accepted(&self) -> u64 {
        self.accepted.load(Ordering::Relaxed)
    }

    pub fn add_dropped(&self) {
        self.dropped.fetch_add(1, Ordering::Relaxed);
    }
    pub fn get_dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn add_timeout(&self) {
        self.timeout.fetch_add(1, Ordering::Relaxed);
    }
    pub fn get_timeout(&self) -> u64 {
        self.timeout.load(Ordering::Relaxed)
    }

    pub fn add_failed(&self) {
        self.failed.fetch_add(1, Ordering::Relaxed);
    }
    pub fn get_failed(&self) -> u64 {
        self.failed.load(Ordering::Relaxed)
    }
}
