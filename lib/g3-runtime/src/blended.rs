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

use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::anyhow;
use tokio::runtime::{Builder, Runtime};

pub struct BlendedRuntimeConfig {
    thread_number: Option<usize>,
    thread_name: Option<String>,
    thread_stack_size: Option<usize>,
    max_io_events_per_tick: Option<usize>,
}

impl Default for BlendedRuntimeConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl BlendedRuntimeConfig {
    pub const fn new() -> Self {
        BlendedRuntimeConfig {
            thread_number: None,
            thread_name: None,
            thread_stack_size: None,
            max_io_events_per_tick: None,
        }
    }

    pub fn set_thread_number(&mut self, num: usize) {
        self.thread_number = Some(num);
    }

    pub fn set_thread_name(&mut self, name: impl Into<String>) {
        self.thread_name = Some(name.into());
    }

    pub fn set_thread_stack_size(&mut self, size: usize) {
        self.thread_stack_size = Some(size);
    }

    pub fn set_max_io_events_per_tick(&mut self, capacity: usize) {
        self.max_io_events_per_tick = Some(capacity);
    }

    pub fn start(&self) -> anyhow::Result<Runtime> {
        let mut build = if let Some(thread_number) = self.thread_number {
            if thread_number == 0 {
                // 0 means no thread pool
                Builder::new_current_thread()
            } else {
                let mut builder = Builder::new_multi_thread();
                builder.worker_threads(thread_number);
                builder
            }
        } else {
            Builder::new_multi_thread()
        };
        build.enable_all();
        let thread_name = self
            .thread_name
            .as_ref()
            .map(|s| s.to_owned())
            .unwrap_or_else(|| "main".to_string());
        build.thread_name_fn(move || {
            static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
            let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
            format!("{thread_name}#{id}")
        });
        if let Some(thread_stack_size) = self.thread_stack_size {
            build.thread_stack_size(thread_stack_size);
        }
        if let Some(n) = self.max_io_events_per_tick {
            build.max_io_events_per_tick(n);
        }
        build
            .build()
            .map_err(|e| anyhow!("runtime build failed: {e:?}"))
    }
}
