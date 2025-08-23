/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::atomic::{AtomicBool, Ordering};

use crate::module::keyless::KeylessRelayStats;

pub(super) struct KeylessTaskStats {
    idle: AtomicBool,
    pub(super) relay: KeylessRelayStats,
}

impl Default for KeylessTaskStats {
    fn default() -> Self {
        KeylessTaskStats {
            idle: AtomicBool::new(false),
            relay: KeylessRelayStats::default(),
        }
    }
}

impl KeylessTaskStats {
    pub(super) fn check_idle(&self) -> bool {
        self.idle.swap(true, Ordering::Relaxed)
    }

    pub(super) fn mark_active(&self) {
        self.idle.store(false, Ordering::Relaxed)
    }
}
