/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use anyhow::anyhow;
use indicatif::{HumanDuration, ProgressBar, ProgressStyle};

#[derive(Clone)]
pub(crate) struct ProgressCounter(Arc<AtomicU64>);

impl ProgressCounter {
    fn new() -> Self {
        ProgressCounter(Arc::new(AtomicU64::new(0)))
    }

    pub(crate) fn inc(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

impl Default for ProgressCounter {
    fn default() -> Self {
        ProgressCounter::new()
    }
}

pub(crate) struct BenchProgress {
    progress: ProgressBar,
    counter: ProgressCounter,
}

impl BenchProgress {
    fn with_progress(progress: ProgressBar) -> Self {
        BenchProgress {
            progress,
            counter: ProgressCounter::new(),
        }
    }

    pub(crate) fn new_fixed(requests: usize) -> Self {
        let bar = ProgressBar::new(requests as u64).with_style(
            ProgressStyle::default_bar()
                .progress_chars("=>-")
                .template("[{elapsed_precise}] {wide_bar} {pos}/{len} ({per_sec})")
                .unwrap(),
        );
        Self::with_progress(bar)
    }

    pub(crate) fn new_timed(timeout: Duration) -> Self {
        let template = format!(
            "[{{elapsed}}/{}] Total: {{pos}}, Success Rate: {{per_sec}}",
            HumanDuration(timeout)
        );
        let bar = ProgressBar::new_spinner().with_style(
            ProgressStyle::default_spinner()
                .template(&template)
                .unwrap(),
        );
        Self::with_progress(bar)
    }

    #[inline]
    pub(crate) fn counter(&self) -> ProgressCounter {
        self.counter.clone()
    }

    #[inline]
    pub(crate) fn finish(self) {
        self.progress.finish_and_clear();
    }

    pub(crate) fn spawn(self, quit: Arc<AtomicBool>) -> anyhow::Result<thread::JoinHandle<Self>> {
        thread::Builder::new()
            .name("progress-bar".to_string())
            .spawn(move || {
                loop {
                    let delta = self.counter.0.swap(0, Ordering::Relaxed);
                    self.progress.inc(delta);

                    if quit.load(Ordering::Relaxed) {
                        break;
                    }

                    thread::sleep(Duration::from_millis(200));
                }
                self
            })
            .map_err(|e| anyhow!("failed to create progress bar thread: {e}"))
    }
}
