/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::atomic::{AtomicU32, Ordering};

static ATOMIC_STAT_ID: AtomicU32 = AtomicU32::new(1); // start from 1

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct StatId {
    pid: u32,
    aid: u32,
}

impl StatId {
    /// Create a StatId that is unique in current process
    pub fn new_unique() -> Self {
        StatId {
            pid: std::process::id(),
            aid: ATOMIC_STAT_ID.fetch_add(1, Ordering::Relaxed),
        }
    }

    pub fn as_u64(&self) -> u64 {
        ((self.pid as u64) << 32) | (self.aid as u64)
    }
}
