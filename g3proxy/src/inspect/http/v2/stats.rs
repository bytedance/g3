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

use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};

pub(crate) struct H2ConcurrencyStats {
    total_task: AtomicU64,
    alive_task: AtomicI32,
}

impl Default for H2ConcurrencyStats {
    fn default() -> Self {
        H2ConcurrencyStats {
            total_task: AtomicU64::new(0),
            alive_task: AtomicI32::new(0),
        }
    }
}

impl H2ConcurrencyStats {
    pub(super) fn add_task(&self) {
        self.total_task.fetch_add(1, Ordering::Relaxed);
        self.alive_task.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn del_task(&self) {
        self.alive_task.fetch_sub(1, Ordering::Relaxed);
    }

    pub(super) fn get_total_task(&self) -> u64 {
        self.total_task.load(Ordering::Relaxed)
    }

    pub(super) fn get_alive_task(&self) -> i32 {
        self.alive_task.load(Ordering::Relaxed)
    }
}
