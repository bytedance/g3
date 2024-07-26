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
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::time::Instant;

use g3_types::limit::GlobalDatagramSpeedLimitConfig;

use crate::limit::{DatagramLimitAction, GlobalDatagramLimit};

pub struct GlobalDatagramLimiter {
    config: ArcSwap<GlobalDatagramSpeedLimitConfig>,
    byte_tokens: AtomicU64,
    packet_tokens: AtomicU64,
    last_updated: ArcSwap<Instant>,
}

impl GlobalDatagramLimiter {
    pub fn new(config: GlobalDatagramSpeedLimitConfig) -> Self {
        GlobalDatagramLimiter {
            config: ArcSwap::new(Arc::new(config)),
            byte_tokens: AtomicU64::new(config.replenish_bytes()),
            packet_tokens: AtomicU64::new(config.replenish_packets()),
            last_updated: ArcSwap::new(Arc::new(Instant::now())),
        }
    }

    pub fn update(&self, config: GlobalDatagramSpeedLimitConfig) {
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
                self.add_packets(config.replenish_packets(), config.max_burst_packets());
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

    fn add_packets(&self, count: u64, max_burst: u64) {
        let mut cur_tokens = self.packet_tokens.load(Ordering::Acquire);

        loop {
            if cur_tokens >= max_burst {
                break;
            }
            let next_tokens = (cur_tokens + count).max(max_burst);
            match self.packet_tokens.compare_exchange(
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

impl GlobalDatagramLimit for GlobalDatagramLimiter {
    fn check_packet(&self, buf_size: usize) -> DatagramLimitAction {
        let config = *self.config.load().as_ref();

        if config.replenish_packets() > 0 {
            let mut cur_tokens = self.packet_tokens.load(Ordering::Acquire);

            loop {
                if cur_tokens == 0 {
                    return DatagramLimitAction::DelayUntil(self.wait_until());
                }
                let left_tokens = cur_tokens.saturating_sub(1);
                match self.packet_tokens.compare_exchange(
                    cur_tokens,
                    left_tokens,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break,
                    Err(actual) => cur_tokens = actual,
                }
            }
        }

        if config.replenish_bytes() > 0 {
            let mut cur_tokens = self.byte_tokens.load(Ordering::Acquire);

            loop {
                if cur_tokens < buf_size as u64 {
                    if config.replenish_packets() > 0 {
                        self.add_packets(1, config.max_burst_packets());
                    }
                    return DatagramLimitAction::DelayUntil(self.wait_until());
                }
                let left_tokens = cur_tokens - buf_size as u64;
                match self.byte_tokens.compare_exchange(
                    cur_tokens,
                    left_tokens,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break,
                    Err(actual) => cur_tokens = actual,
                }
            }
        }

        DatagramLimitAction::Advance(1)
    }

    fn check_packets(&self, total_size_v: &[usize]) -> DatagramLimitAction {
        let config = *self.config.load().as_ref();

        let mut to_advance = total_size_v.len();
        if config.replenish_packets() > 0 {
            let mut cur_tokens = self.packet_tokens.load(Ordering::Acquire);

            loop {
                if cur_tokens == 0 {
                    return DatagramLimitAction::DelayUntil(self.wait_until());
                }
                let left_tokens = cur_tokens.saturating_sub(to_advance as u64);
                match self.packet_tokens.compare_exchange(
                    cur_tokens,
                    left_tokens,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => to_advance = (cur_tokens - left_tokens) as usize,
                    Err(actual) => cur_tokens = actual,
                }
            }
        }

        let mut buf_size = total_size_v[to_advance - 1];
        if config.replenish_bytes() > 0 {
            let mut cur_tokens = self.byte_tokens.load(Ordering::Acquire);

            loop {
                if cur_tokens == 0 {
                    if config.replenish_packets() > 0 {
                        self.add_packets(to_advance as u64, config.max_burst_packets());
                    }
                    return DatagramLimitAction::DelayUntil(self.wait_until());
                }
                let left_tokens = cur_tokens.saturating_sub(buf_size as u64);
                match self.byte_tokens.compare_exchange(
                    cur_tokens,
                    left_tokens,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => buf_size = (cur_tokens - left_tokens) as usize,
                    Err(actual) => cur_tokens = actual,
                }
            }
        }

        if buf_size == total_size_v[to_advance - 1] {
            return DatagramLimitAction::Advance(to_advance);
        }

        match total_size_v.binary_search(&buf_size) {
            Ok(found_index) => {
                if config.replenish_packets() > 0 {
                    // release unneeded packets
                    self.add_packets(
                        (to_advance - found_index - 1) as u64,
                        config.max_burst_packets(),
                    );
                }
                to_advance = found_index + 1;
            }
            Err(insert_index) => {
                if config.replenish_packets() > 0 {
                    // release unneeded packets
                    self.add_packets(
                        (to_advance - insert_index) as u64,
                        config.max_burst_packets(),
                    );
                }
                to_advance = insert_index;
                if config.replenish_bytes() > 0 {
                    // release unneeded bytes
                    self.add_bytes(
                        (buf_size - total_size_v[to_advance - 1]) as u64,
                        config.max_burst_bytes(),
                    );
                }
            }
        }
        DatagramLimitAction::Advance(to_advance)
    }

    fn release_bytes(&self, size: usize) {
        let max_burst = self.config.load().as_ref().max_burst_bytes();
        self.add_bytes(size as u64, max_burst);
    }

    fn release_packets(&self, count: usize) {
        let max_burst = self.config.load().as_ref().max_burst_packets();
        self.add_packets(count as u64, max_burst);
    }
}
