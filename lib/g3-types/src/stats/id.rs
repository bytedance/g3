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

use std::sync::atomic::{AtomicU32, Ordering};

static ATOMIC_STAT_ID: AtomicU32 = AtomicU32::new(1); // start from 1

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct StatId {
    pid: u32,
    aid: u32,
}

impl StatId {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        StatId {
            pid: std::process::id(),
            aid: ATOMIC_STAT_ID.fetch_add(1, Ordering::Relaxed),
        }
    }

    pub fn as_u64(&self) -> u64 {
        ((self.pid as u64) << 32) | (self.aid as u64)
    }
}
