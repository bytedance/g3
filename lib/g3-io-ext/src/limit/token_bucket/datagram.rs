/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

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
            config: ArcSwap::from_pointee(config),
            byte_tokens: AtomicU64::new(config.replenish_bytes()),
            packet_tokens: AtomicU64::new(config.replenish_packets()),
            last_updated: ArcSwap::from_pointee(Instant::now()),
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
                let replenish_bytes = config.replenish_bytes();
                if replenish_bytes > 0 {
                    self.add_bytes(replenish_bytes, config.max_burst_bytes());
                }
                let replenish_packets = config.replenish_packets();
                if replenish_packets > 0 {
                    self.add_packets(replenish_packets, config.max_burst_packets());
                }
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

    fn add_packets(&self, count: u64, max_burst: u64) {
        let mut cur_tokens = self.packet_tokens.load(Ordering::Acquire);

        loop {
            if cur_tokens >= max_burst {
                break;
            }
            let next_tokens = (cur_tokens + count).min(max_burst);
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
                    Ok(_) => {
                        to_advance = (cur_tokens - left_tokens) as usize;
                        break;
                    }
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
                    Ok(_) => {
                        buf_size = (cur_tokens - left_tokens) as usize;
                        break;
                    }
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
        if max_burst > 0 {
            self.add_bytes(size as u64, max_burst);
        }
    }

    fn release_packets(&self, count: usize) {
        let max_burst = self.config.load().as_ref().max_burst_packets();
        if max_burst > 0 {
            self.add_packets(count as u64, max_burst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_packet() {
        let config = GlobalDatagramSpeedLimitConfig::per_second(1000);
        let limiter = GlobalDatagramLimiter::new(config);
        assert_eq!(limiter.check_packet(100), DatagramLimitAction::Advance(1));
        assert_eq!(limiter.check_packet(900), DatagramLimitAction::Advance(1));
        assert_ne!(limiter.check_packet(100), DatagramLimitAction::Advance(1));
    }

    #[test]
    fn check_packets() {
        let config = GlobalDatagramSpeedLimitConfig::per_second(1000);
        let limiter = GlobalDatagramLimiter::new(config);
        let total_len_v = [100, 200];
        assert_eq!(
            limiter.check_packets(&total_len_v),
            DatagramLimitAction::Advance(2)
        );

        let total_len_v = [200, 700, 900];
        assert_eq!(
            limiter.check_packets(&total_len_v),
            DatagramLimitAction::Advance(2)
        );

        let total_len_v = [50, 40, 10];
        assert_eq!(
            limiter.check_packets(&total_len_v),
            DatagramLimitAction::Advance(3)
        );
    }
}
