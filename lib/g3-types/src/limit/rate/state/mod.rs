/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

mod local;

mod global;

pub use global::GlobalRateLimitState;

pub trait RateLimitState {
    fn fetch_and_update<F>(&self, update: F) -> Result<(), Duration>
    where
        F: Fn(u64) -> Result<u64, Duration>;
}
