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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionPoolConfig {
    check_interval: Duration,
    max_idle_count: usize,
    min_idle_count: usize,
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
}
