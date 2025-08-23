/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::{Duration, Instant};

use g3_std_ext::time::DurationExt;

mod quota;
pub use quota::RateLimitQuota;

mod state;
pub use state::{GlobalRateLimitState, RateLimitState};

/// A RateLimiter based on GCRA
///
/// [GCRA](https://en.wikipedia.org/wiki/Generic_cell_rate_algorithm)
pub struct RateLimiter<S> {
    start: Instant,
    replenish_nanos: u64,
    max_burst_nanos: u64,
    state: S,
}

impl<S> RateLimiter<S> {
    pub fn with_state(quota: RateLimitQuota, state: S) -> RateLimiter<S> {
        let replenish_nanos = quota.replenish_nanos.get();
        let max_burst_nanos = (quota.max_burst.get() as u64 - 1).saturating_mul(replenish_nanos);
        let start = Instant::now();
        RateLimiter {
            start,
            replenish_nanos,
            max_burst_nanos,
            state,
        }
    }
}

impl<S: RateLimitState> RateLimiter<S> {
    fn check_with_t(&self, now_nanos: u64) -> Result<(), Duration> {
        self.state.fetch_and_update(|tat| {
            let earliest_nanos = tat.saturating_sub(self.max_burst_nanos);
            if now_nanos < earliest_nanos {
                Err(Duration::from_nanos(earliest_nanos - now_nanos))
            } else {
                Ok(tat.max(now_nanos) + self.replenish_nanos)
            }
        })
    }

    pub fn check(&self) -> Result<(), Duration> {
        let now_nanos = self.start.elapsed().as_nanos_u64();
        self.check_with_t(now_nanos)
    }
}

impl RateLimiter<GlobalRateLimitState> {
    pub fn new_global(quota: RateLimitQuota) -> Self {
        Self::with_state(quota, GlobalRateLimitState::default())
    }
}

impl RateLimiter<Arc<GlobalRateLimitState>> {
    pub fn new_global_reloadable(quota: RateLimitQuota) -> Self {
        Self::with_state(quota, Arc::new(GlobalRateLimitState::default()))
    }

    pub fn reload(&self, quota: RateLimitQuota) -> Self {
        let replenish_nanos = quota.replenish_nanos.get();
        let max_burst_nanos = (quota.max_burst.get() as u64).saturating_mul(replenish_nanos);
        RateLimiter {
            start: self.start,
            replenish_nanos,
            max_burst_nanos,
            state: self.state.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU32;

    #[test]
    fn normal() {
        let mut quota = RateLimitQuota::with_period(Duration::from_nanos(5)).unwrap();
        quota.allow_burst(NonZeroU32::new(4).unwrap());

        let rate_limiter = RateLimiter::new_global(quota);
        assert_eq!(rate_limiter.replenish_nanos, 5);
        assert_eq!(rate_limiter.max_burst_nanos, 15);
        assert_eq!(rate_limiter.state.target_t(), 0);

        // TAT = 0
        assert!(rate_limiter.check_with_t(10).is_ok());
        assert_eq!(rate_limiter.state.target_t(), 15);
        // TAT = 15
        assert!(rate_limiter.check_with_t(11).is_ok());
        assert_eq!(rate_limiter.state.target_t(), 20);
        // TAT = 20
        assert!(rate_limiter.check_with_t(12).is_ok());
        assert_eq!(rate_limiter.state.target_t(), 25);
        // TAT = 25
        assert!(rate_limiter.check_with_t(12).is_ok());
        assert_eq!(rate_limiter.state.target_t(), 30);
        // TAT = 30
        assert!(rate_limiter.check_with_t(14).is_err());
        assert_eq!(rate_limiter.state.target_t(), 30);

        // TAT = 30
        assert!(rate_limiter.check_with_t(15).is_ok());
        assert_eq!(rate_limiter.state.target_t(), 35);
        // TAT = 35
        assert!(rate_limiter.check_with_t(21).is_ok());
        assert_eq!(rate_limiter.state.target_t(), 40);
        // TAT = 40
        let wait = rate_limiter.check_with_t(21).unwrap_err();
        assert_eq!(wait, Duration::from_nanos(4));
        let wait = rate_limiter.check_with_t(21).unwrap_err();
        assert_eq!(wait, Duration::from_nanos(4));
        assert_eq!(rate_limiter.state.target_t(), 40);

        // TAT = 40
        assert!(rate_limiter.check_with_t(25).is_ok());
        assert_eq!(rate_limiter.state.target_t(), 45);
    }

    #[test]
    fn max_burst_1() {
        let mut quota = RateLimitQuota::with_period(Duration::from_nanos(5)).unwrap();
        quota.allow_burst(NonZeroU32::MIN);

        let rate_limiter = RateLimiter::new_global(quota);
        assert_eq!(rate_limiter.replenish_nanos, 5);
        assert_eq!(rate_limiter.max_burst_nanos, 0);
        assert_eq!(rate_limiter.state.target_t(), 0);

        // TAT = 0
        assert!(rate_limiter.check_with_t(10).is_ok());
        assert_eq!(rate_limiter.state.target_t(), 15);
        let wait = rate_limiter.check_with_t(11).unwrap_err();
        assert_eq!(wait, Duration::from_nanos(4));
        assert_eq!(rate_limiter.state.target_t(), 15);

        // TAT = 15
        assert!(rate_limiter.check_with_t(15).is_ok());
        assert_eq!(rate_limiter.state.target_t(), 20);
    }
}
