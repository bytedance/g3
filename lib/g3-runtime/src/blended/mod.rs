/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::anyhow;
use tokio::runtime::{Builder, Runtime};

#[cfg(feature = "yaml")]
mod yaml;

#[derive(Clone)]
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

    pub fn intended_thread_number(&self) -> usize {
        self.thread_number
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|v| v.get())
                    .unwrap_or(1)
            })
            .max(1)
    }

    pub fn run_in_current_thread(&self) -> bool {
        self.thread_number == Some(0)
    }

    pub fn set_thread_number(&mut self, num: usize) {
        self.thread_number = Some(num);
    }

    pub fn set_default_thread_number(&mut self, num: usize) {
        if self.thread_name.is_none() {
            self.thread_number = Some(num);
        }
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

    pub fn builder(&self) -> Builder {
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
    }

    pub fn start(&self) -> anyhow::Result<Runtime> {
        self.builder()
            .build()
            .map_err(|e| anyhow!("runtime build failed: {e:?}"))
    }
}
