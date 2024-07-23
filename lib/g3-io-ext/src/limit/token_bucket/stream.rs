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

use std::sync::atomic::{AtomicU64, Ordering};

use arc_swap::ArcSwap;
use tokio::time::Instant;

use g3_types::limit::GlobalStreamSpeedLimitConfig;

use crate::limit::{GlobalStreamLimit, StreamLimitAction};

pub struct GlobalStreamLimiter {
    config: ArcSwap<GlobalStreamSpeedLimitConfig>,
    byte_tokens: AtomicU64,
    last_updated: ArcSwap<Instant>,
}

impl GlobalStreamLimiter {
    fn add_bytes(&self, size: u64, max_burst: u64) {
        let mut cur_tokens = self.byte_tokens.load(Ordering::Acquire);

        loop {
            if cur_tokens >= max_burst {
                break;
            }
            let next_tokens = (cur_tokens + size).max(max_burst);
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
}

impl GlobalStreamLimit for GlobalStreamLimiter {
    fn check(&self, to_advance: usize) -> StreamLimitAction {
        let mut cur_tokens = self.byte_tokens.load(Ordering::Acquire);

        loop {
            if cur_tokens == 0 {
                return StreamLimitAction::DelayUntil(self.wait_until());
            }
            let left_tokens = cur_tokens.saturating_sub(to_advance as u64);
            match self.byte_tokens.compare_exchange(
                cur_tokens,
                left_tokens,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return StreamLimitAction::AdvanceBy((cur_tokens - left_tokens) as usize),
                Err(actual) => cur_tokens = actual,
            }
        }
    }

    fn release(&self, size: usize) {
        let max_burst = self.config.load().as_ref().max_burst_bytes();
        self.add_bytes(size as u64, max_burst);
    }
}
