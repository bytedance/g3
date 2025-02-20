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

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use arc_swap::ArcSwap;
use tokio::time::Instant;

use g3_types::limit::GlobalStreamSpeedLimitConfig;

use crate::limit::{GlobalLimitGroup, GlobalStreamLimit, StreamLimitAction};

pub struct GlobalStreamLimiter {
    group: GlobalLimitGroup,
    config: ArcSwap<GlobalStreamSpeedLimitConfig>,
    byte_tokens: AtomicU64,
    last_updated: ArcSwap<Instant>,
}

impl GlobalStreamLimiter {
    pub fn new(group: GlobalLimitGroup, config: GlobalStreamSpeedLimitConfig) -> Self {
        GlobalStreamLimiter {
            group,
            config: ArcSwap::new(Arc::new(config)),
            byte_tokens: AtomicU64::new(config.replenish_bytes()),
            last_updated: ArcSwap::new(Arc::new(Instant::now())),
        }
    }

    pub fn update(&self, config: GlobalStreamSpeedLimitConfig) {
        self.config.store(Arc::new(config));
    }

    pub fn tokio_spawn_replenish(self: Arc<Self>) {
        let fut = async move {
            loop {
                if Arc::strong_count(&self) <= 1 {
                    break;
                }
                let config = *self.config.load().as_ref();
                tokio::time::sleep(config.replenish_interval()).await;
                self.add_bytes(config.replenish_bytes(), config.max_burst_bytes());
                self.last_updated.store(Arc::new(Instant::now()));
            }
        };
        if let Some(handle) = crate::limit::get_limit_schedule_rt_handle() {
            handle.spawn(fut);
        } else {
            tokio::spawn(fut);
        }
    }

    fn add_bytes(&self, size: u64, max_burst: u64) {
        let mut cur_tokens = self.byte_tokens.load(Ordering::Acquire);

        loop {
            if cur_tokens >= max_burst {
                break;
            }
            let next_tokens = (cur_tokens + size).min(max_burst);
            match self.byte_tokens.compare_exchange(
                cur_tokens,
                next_tokens,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => cur_tokens = actual,
            }
        }
    }

    fn wait_until(&self) -> Instant {
        let last_updated = *self.last_updated.load().as_ref();
        let interval = self.config.load().as_ref().replenish_interval();
        last_updated + interval
    }

    pub fn try_consume(&self, size: u64) -> Option<u64> {
        let mut cur_tokens = self.byte_tokens.load(Ordering::Acquire);

        loop {
            if cur_tokens == 0 {
                return None;
            }
            let left_tokens = cur_tokens.saturating_sub(size);
            match self.byte_tokens.compare_exchange(
                cur_tokens,
                left_tokens,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(cur_tokens - left_tokens),
                Err(actual) => cur_tokens = actual,
            }
        }
    }
}

impl GlobalStreamLimit for GlobalStreamLimiter {
    fn group(&self) -> GlobalLimitGroup {
        self.group
    }

    fn check(&self, to_advance: usize) -> StreamLimitAction {
        match self.try_consume(to_advance as u64) {
            Some(n) => StreamLimitAction::AdvanceBy(n as usize),
            None => StreamLimitAction::DelayUntil(self.wait_until()),
        }
    }

    fn release(&self, size: usize) {
        let max_burst = self.config.load().as_ref().max_burst_bytes();
        self.add_bytes(size as u64, max_burst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check() {
        let config = GlobalStreamSpeedLimitConfig::per_second(1000);
        let limiter = GlobalStreamLimiter::new(GlobalLimitGroup::Server, config);
        assert_eq!(limiter.check(100), StreamLimitAction::AdvanceBy(100));
        assert_eq!(limiter.check(900), StreamLimitAction::AdvanceBy(900));
        limiter.release(100);
        assert_eq!(limiter.check(1000), StreamLimitAction::AdvanceBy(100));
    }
}
