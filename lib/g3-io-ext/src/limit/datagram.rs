/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use tokio::time::Instant;

use super::LocalDatagramLimiter;

#[derive(Debug, Eq, PartialEq)]
pub enum DatagramLimitAction {
    Advance(usize),
    DelayUntil(Instant),
    DelayFor(u64),
}

pub trait GlobalDatagramLimit {
    fn check_packet(&self, buf_size: usize) -> DatagramLimitAction;
    fn check_packets(&self, total_size_v: &[usize]) -> DatagramLimitAction;
    fn release_bytes(&self, size: usize);
    fn release_packets(&self, count: usize);
}

struct GlobalLimiter {
    inner: Arc<dyn GlobalDatagramLimit + Send + Sync>,
    checked_packets: Option<usize>,
    checked_bytes: Option<usize>,
}

impl GlobalLimiter {
    fn new<T>(inner: Arc<T>) -> Self
    where
        T: GlobalDatagramLimit + Send + Sync + 'static,
    {
        GlobalLimiter {
            inner,
            checked_packets: None,
            checked_bytes: None,
        }
    }
}

#[derive(Default)]
pub struct DatagramLimiter {
    is_set: bool,
    local_is_set: bool,
    local: LocalDatagramLimiter,
    global: Vec<GlobalLimiter>,
}

impl DatagramLimiter {
    pub fn with_local(shift_millis: u8, max_packets: usize, max_bytes: usize) -> Self {
        let local = LocalDatagramLimiter::new(shift_millis, max_packets, max_bytes);
        let local_is_set = local.is_set();
        DatagramLimiter {
            is_set: local_is_set,
            local_is_set,
            local,
            global: Vec::new(),
        }
    }

    pub fn reset_local(
        &mut self,
        shift_millis: u8,
        max_packets: usize,
        max_bytes: usize,
        cur_millis: u64,
    ) {
        self.local
            .reset(shift_millis, max_packets, max_bytes, cur_millis);
        self.local_is_set = self.local.is_set();
        if self.global.is_empty() {
            self.is_set = self.local_is_set;
        }
    }

    pub fn add_global<T>(&mut self, limiter: Arc<T>)
    where
        T: GlobalDatagramLimit + Send + Sync + 'static,
    {
        self.global.push(GlobalLimiter::new(limiter));
        self.is_set = true;
    }

    #[inline]
    pub fn is_set(&self) -> bool {
        self.is_set
    }

    pub fn check_packet(&mut self, cur_millis: u64, buf_size: usize) -> DatagramLimitAction {
        if self.local_is_set {
            match self.local.check_packet(cur_millis, buf_size) {
                DatagramLimitAction::Advance(_) => {}
                DatagramLimitAction::DelayUntil(t) => return DatagramLimitAction::DelayUntil(t),
                DatagramLimitAction::DelayFor(n) => {
                    return DatagramLimitAction::DelayFor(n);
                }
            }
        }

        for limiter in &mut self.global {
            match limiter.inner.check_packet(buf_size) {
                DatagramLimitAction::Advance(_) => {
                    limiter.checked_packets = Some(1);
                    limiter.checked_bytes = Some(buf_size);
                }
                DatagramLimitAction::DelayUntil(t) => {
                    self.release_global();
                    return DatagramLimitAction::DelayUntil(t);
                }
                DatagramLimitAction::DelayFor(n) => {
                    self.release_global();
                    return DatagramLimitAction::DelayFor(n);
                }
            }
        }

        DatagramLimitAction::Advance(1)
    }

    pub fn check_packets(
        &mut self,
        cur_millis: u64,
        total_size_v: &[usize],
    ) -> DatagramLimitAction {
        let mut to_advance = if self.local_is_set {
            match self.local.check_packets(cur_millis, total_size_v) {
                DatagramLimitAction::Advance(n) => n,
                DatagramLimitAction::DelayUntil(t) => return DatagramLimitAction::DelayUntil(t),
                DatagramLimitAction::DelayFor(n) => {
                    return DatagramLimitAction::DelayFor(n);
                }
            }
        } else {
            total_size_v.len()
        };
        if self.global.is_empty() {
            return DatagramLimitAction::Advance(to_advance);
        }

        for limiter in &mut self.global {
            match limiter.inner.check_packets(&total_size_v[..to_advance]) {
                DatagramLimitAction::Advance(n) => {
                    to_advance = n;
                    limiter.checked_packets = Some(n);
                }
                DatagramLimitAction::DelayUntil(t) => {
                    self.release_global();
                    return DatagramLimitAction::DelayUntil(t);
                }
                DatagramLimitAction::DelayFor(n) => {
                    self.release_global();
                    return DatagramLimitAction::DelayFor(n);
                }
            }
        }

        if total_size_v.len() > to_advance {
            let buf_size = total_size_v[to_advance - 1];
            for limiter in &mut self.global {
                let checked = limiter.checked_packets.take().unwrap();
                if checked > to_advance {
                    limiter.inner.release_packets(checked - to_advance);
                    limiter
                        .inner
                        .release_bytes(total_size_v[checked - 1] - buf_size);
                }
                limiter.checked_packets = Some(to_advance);
                limiter.checked_bytes = Some(buf_size);
            }
        }
        DatagramLimitAction::Advance(to_advance)
    }

    pub fn release_global(&mut self) {
        for limiter in &mut self.global {
            let Some(packets) = limiter.checked_packets.take() else {
                break;
            };
            limiter.inner.release_packets(packets);
            if let Some(size) = limiter.checked_bytes.take() {
                limiter.inner.release_bytes(size);
            }
        }
    }

    pub fn set_advance(&mut self, packets: usize, size: usize) {
        if self.local_is_set {
            self.local.set_advance(packets, size);
        }

        for limiter in &mut self.global {
            let Some(checked) = limiter.checked_packets.take() else {
                break;
            };

            if checked > packets {
                limiter.inner.release_packets(checked - packets);
            }

            if let Some(checked) = limiter.checked_bytes.take() {
                if checked > size {
                    limiter.inner.release_bytes(checked - size);
                }
            }
        }
    }
}
