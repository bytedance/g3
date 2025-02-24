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

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use hdrhistogram::Histogram;

static GLOBAL_STATE: GlobalState = GlobalState::new(None, 0);

pub(super) fn global_state() -> &'static GlobalState {
    &GLOBAL_STATE
}

pub(super) fn mark_force_quit() {
    GLOBAL_STATE.mark_force_quit();
}

pub(super) fn init_global_state(requests: Option<usize>, log_error_count: usize) {
    GLOBAL_STATE
        .check_total
        .store(requests.is_some(), Ordering::Relaxed);
    GLOBAL_STATE
        .total_left
        .store(requests.unwrap_or_default(), Ordering::Relaxed);
    GLOBAL_STATE
        .log_error_left
        .store(log_error_count, Ordering::Relaxed);
}

pub(super) struct GlobalState {
    check_total: AtomicBool,
    force_quit: AtomicBool,
    total_left: AtomicUsize,
    total_passed: AtomicUsize,
    total_failed: AtomicUsize,
    log_error_left: AtomicUsize,
    request_id: AtomicUsize,
}

impl Default for GlobalState {
    fn default() -> Self {
        GlobalState::new(None, 0)
    }
}

impl GlobalState {
    pub(super) const fn new(requests: Option<usize>, log_error_count: usize) -> Self {
        let total_left = match requests {
            Some(n) => AtomicUsize::new(n),
            None => AtomicUsize::new(0),
        };
        GlobalState {
            check_total: AtomicBool::new(requests.is_some()),
            force_quit: AtomicBool::new(false),
            total_left,
            total_passed: AtomicUsize::new(0),
            total_failed: AtomicUsize::new(0),
            log_error_left: AtomicUsize::new(log_error_count),
            request_id: AtomicUsize::new(0),
        }
    }

    fn mark_force_quit(&self) {
        self.force_quit.store(true, Ordering::Relaxed);
    }

    pub(super) fn fetch_request(&self) -> Option<usize> {
        if self.force_quit.load(Ordering::Relaxed) {
            return None;
        }

        if self.check_total.load(Ordering::Relaxed) {
            let mut curr = self.total_left.load(Ordering::Acquire);
            loop {
                if curr == 0 {
                    return None;
                }

                match self.total_left.compare_exchange(
                    curr,
                    curr - 1,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break,
                    Err(actual) => curr = actual,
                }
            }
        }

        Some(self.request_id.fetch_add(1, Ordering::Relaxed))
    }

    pub(super) fn check_log_error(&self) -> bool {
        let mut curr = self.log_error_left.load(Ordering::Acquire);
        loop {
            if curr == 0 {
                return false;
            }

            match self.log_error_left.compare_exchange(
                curr,
                curr - 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(actual) => curr = actual,
            }
        }
    }

    pub(super) fn add_passed(&self) {
        self.total_passed.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn add_failed(&self) {
        self.total_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn all_succeeded(&self) -> bool {
        self.total_failed.load(Ordering::Relaxed) == 0
    }

    pub(super) fn summary(&self, total_time: Duration, distribution: &Histogram<u64>) {
        println!("Time taken for tests: {total_time:?}");

        let passed = self.total_passed.load(Ordering::Relaxed);
        println!("Complete requests:    {passed:<10}");

        let failed = self.total_failed.load(Ordering::Relaxed);
        if failed > 0 {
            println!("Failed requests:      {failed}");
        }

        let left = self.total_left.load(Ordering::Relaxed);
        if left > 0 {
            println!("Left requests:        {left}");
        }

        println!(
            "Requests per second:  {:.3} [#/sec] (mean)",
            passed as f64 / total_time.as_secs_f64()
        );

        println!("Requests distribution:");
        println!("  min   {}", distribution.min());
        println!(
            "  mean  {:.2}[+/- {:.2}]",
            distribution.mean(),
            distribution.stdev()
        );
        println!("  pct90 {}", distribution.value_at_percentile(90.0));
        println!("  max   {}", distribution.max());
    }
}
