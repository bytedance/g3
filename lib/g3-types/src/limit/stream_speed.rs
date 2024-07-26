/*
 * Copyright 2024 ByteDance and/or its affiliates.
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
