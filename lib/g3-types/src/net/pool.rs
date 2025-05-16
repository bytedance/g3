/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionPoolConfig {
    check_interval: Duration,
    max_idle_count: usize,
    min_idle_count: usize,
    idle_timeout: Duration,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        ConnectionPoolConfig::new(1024, 32)
    }
}

impl ConnectionPoolConfig {
    pub fn new(max_idle: usize, min_idle: usize) -> Self {
        ConnectionPoolConfig {
            check_interval: Duration::from_secs(10),
            max_idle_count: max_idle,
            min_idle_count: min_idle,
            idle_timeout: Duration::from_secs(300),
        }
    }

    #[inline]
    pub fn set_check_interval(&mut self, interval: Duration) {
        self.check_interval = interval;
    }

    #[inline]
    pub fn check_interval(&self) -> Duration {
        self.check_interval
    }

    #[inline]
    pub fn set_max_idle_count(&mut self, count: usize) {
        self.max_idle_count = count;
    }

    #[inline]
    pub fn max_idle_count(&self) -> usize {
        self.max_idle_count
    }

    #[inline]
    pub fn set_min_idle_count(&mut self, count: usize) {
        self.min_idle_count = count;
    }

    #[inline]
    pub fn min_idle_count(&self) -> usize {
        self.min_idle_count
    }

    #[inline]
    pub fn set_idle_timeout(&mut self, timeout: Duration) {
        self.idle_timeout = timeout;
    }

    #[inline]
    pub fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }
}
