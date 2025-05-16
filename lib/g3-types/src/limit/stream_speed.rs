/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use anyhow::anyhow;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct GlobalStreamSpeedLimitConfig {
    replenish_interval: Duration,
    replenish_bytes: u64,
    max_burst_bytes: u64,
}

impl GlobalStreamSpeedLimitConfig {
    pub fn per_second(size: u64) -> Self {
        GlobalStreamSpeedLimitConfig {
            replenish_interval: Duration::from_secs(1),
            replenish_bytes: size,
            max_burst_bytes: size,
        }
    }

    #[inline]
    pub fn replenish_interval(&self) -> Duration {
        self.replenish_interval
    }

    pub fn set_replenish_interval(&mut self, interval: Duration) {
        self.replenish_interval = interval;
    }

    #[inline]
    pub fn replenish_bytes(&self) -> u64 {
        self.replenish_bytes
    }

    pub fn set_replenish_bytes(&mut self, size: u64) {
        self.replenish_bytes = size;
    }

    #[inline]
    pub fn max_burst_bytes(&self) -> u64 {
        self.max_burst_bytes
    }

    pub fn set_max_burst_bytes(&mut self, size: u64) {
        self.max_burst_bytes = size;
    }

    pub fn check(&mut self) -> anyhow::Result<()> {
        if self.replenish_bytes == 0 {
            return Err(anyhow!("no replenish bytes set"));
        }
        if self.max_burst_bytes < self.replenish_bytes {
            self.max_burst_bytes = self.replenish_bytes;
        }

        Ok(())
    }
}
