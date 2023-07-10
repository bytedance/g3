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

#![allow(unused)]

use std::sync::atomic::{AtomicI32, AtomicIsize, AtomicU64, Ordering};

use g3_types::metrics::MetricsName;
use g3_types::stats::StatId;

pub(crate) struct KeyServerStats {
    name: MetricsName,
    id: StatId,

    online: AtomicIsize,

    task_total: AtomicU64,
    task_alive_count: AtomicI32,
}

impl KeyServerStats {
    pub(crate) fn new(name: &MetricsName) -> Self {
        KeyServerStats {
            name: name.clone(),
            id: StatId::new(),
            online: AtomicIsize::new(0),
            task_total: AtomicU64::new(0),
            task_alive_count: AtomicI32::new(0),
        }
    }

    #[inline]
    fn name(&self) -> &MetricsName {
        &self.name
    }

    #[inline]
    fn stat_id(&self) -> StatId {
        self.id
    }

    pub(crate) fn set_online(&self) {
        self.online.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn set_offline(&self) {
        self.online.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn add_task(&self) {
        self.task_total.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn inc_alive_task(&self) {
        self.task_alive_count.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn dec_alive_task(&self) {
        self.task_alive_count.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn is_online(&self) -> bool {
        self.online.load(Ordering::Relaxed) > 0
    }

    pub(crate) fn get_task_total(&self) -> u64 {
        self.task_total.load(Ordering::Relaxed)
    }

    pub(crate) fn get_alive_count(&self) -> i32 {
        self.task_alive_count.load(Ordering::Relaxed)
    }
}
