/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use tokio::time::Instant;

use super::{GlobalLimitGroup, LocalStreamLimiter};

#[derive(Debug, Eq, PartialEq)]
pub enum StreamLimitAction {
    AdvanceBy(usize),
    DelayUntil(Instant),
    DelayFor(u64),
}

pub trait GlobalStreamLimit {
    fn group(&self) -> GlobalLimitGroup;
    fn check(&self, to_advance: usize) -> StreamLimitAction;
    fn release(&self, size: usize);
}

struct GlobalLimiter {
    inner: Arc<dyn GlobalStreamLimit + Send + Sync>,
    checked_bytes: Option<usize>,
}

impl GlobalLimiter {
    fn new<T>(inner: Arc<T>) -> Self
    where
        T: GlobalStreamLimit + Send + Sync + 'static,
    {
        GlobalLimiter {
            inner,
            checked_bytes: None,
        }
    }
}

impl Drop for GlobalLimiter {
    fn drop(&mut self) {
        if let Some(taken) = self.checked_bytes.take() {
            self.inner.release(taken);
        }
    }
}

#[derive(Default)]
pub struct StreamLimiter {
    is_set: bool,
    local_is_set: bool,
    local: LocalStreamLimiter,
    global: Vec<GlobalLimiter>,
}

impl StreamLimiter {
    pub fn with_local(shift_millis: u8, max_bytes: usize) -> Self {
        let local = LocalStreamLimiter::new(shift_millis, max_bytes);
        let local_is_set = local.is_set();
        StreamLimiter {
            is_set: local_is_set,
            local_is_set,
            local,
            global: Vec::new(),
        }
    }

    pub fn reset_local(&mut self, shift_millis: u8, max_bytes: usize, cur_millis: u64) {
        self.local.reset(shift_millis, max_bytes, cur_millis);
        self.local_is_set = self.local.is_set();
        if self.global.is_empty() {
            self.is_set = self.local_is_set;
        }
    }

    pub fn add_global<T>(&mut self, limiter: Arc<T>)
    where
        T: GlobalStreamLimit + Send + Sync + 'static,
    {
        self.global.push(GlobalLimiter::new(limiter));
        self.is_set = true;
    }

    pub fn remove_global_by_group(&mut self, group: GlobalLimitGroup) {
        self.global.retain(|l| l.inner.group() != group);
    }

    pub fn retain_global_by_group(&mut self, group: GlobalLimitGroup) {
        self.global.retain(|l| l.inner.group() == group);
    }

    #[inline]
    pub fn is_set(&self) -> bool {
        self.is_set
    }

    pub fn check(&mut self, cur_millis: u64, mut to_advance: usize) -> StreamLimitAction {
        let target = to_advance;
        if self.local_is_set {
            to_advance = match self.local.check(cur_millis, to_advance) {
                StreamLimitAction::AdvanceBy(size) => size,
                StreamLimitAction::DelayUntil(t) => return StreamLimitAction::DelayUntil(t),
                StreamLimitAction::DelayFor(n) => return StreamLimitAction::DelayFor(n),
            }
        }

        for limiter in &mut self.global {
            match limiter.inner.check(to_advance) {
                StreamLimitAction::AdvanceBy(size) => {
                    to_advance = size;
                    limiter.checked_bytes = Some(size);
                }
                StreamLimitAction::DelayUntil(t) => {
                    self.release_global();
                    return StreamLimitAction::DelayUntil(t);
                }
                StreamLimitAction::DelayFor(n) => {
                    self.release_global();
                    return StreamLimitAction::DelayFor(n);
                }
            }
        }

        if target > to_advance {
            // shrink in time
            for limiter in &mut self.global {
                let checked = limiter.checked_bytes.take().unwrap();
                if checked > to_advance {
                    limiter.inner.release(checked - to_advance);
                }
                limiter.checked_bytes = Some(to_advance);
            }
        }
        StreamLimitAction::AdvanceBy(to_advance)
    }

    pub fn release_global(&mut self) {
        for limiter in &mut self.global {
            let Some(taken) = limiter.checked_bytes.take() else {
                break;
            };
            limiter.inner.release(taken);
        }
    }

    pub fn set_advance(&mut self, size: usize) {
        if self.local_is_set {
            self.local.set_advance(size);
        }

        for limiter in &mut self.global {
            let Some(taken) = limiter.checked_bytes.take() else {
                break;
            };
            if taken > size {
                limiter.inner.release(taken - size);
            }
        }
    }
}
