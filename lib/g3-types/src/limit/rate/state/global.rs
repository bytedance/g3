/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::RateLimitState;

#[derive(Default)]
pub struct GlobalRateLimitState(AtomicU64);

impl GlobalRateLimitState {
    #[cfg(test)]
    pub(crate) fn target_t(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

impl RateLimitState for GlobalRateLimitState {
    fn fetch_and_update<F>(&self, update: F) -> Result<(), Duration>
    where
        F: Fn(u64) -> Result<u64, Duration>,
    {
        let mut cur = self.0.load(Ordering::Acquire);
        let mut decision = update(cur)?;
        loop {
            match self
                .0
                .compare_exchange_weak(cur, decision, Ordering::Acquire, Ordering::Relaxed)
            {
                Ok(_) => return Ok(()),
                Err(v) => cur = v,
            }
            decision = update(cur)?;
        }
    }
}

impl RateLimitState for Arc<GlobalRateLimitState> {
    fn fetch_and_update<F>(&self, update: F) -> Result<(), Duration>
    where
        F: Fn(u64) -> Result<u64, Duration>,
    {
        self.as_ref().fetch_and_update(update)
    }
}
