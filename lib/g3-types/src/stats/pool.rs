/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, AtomicU64, Ordering};

#[derive(Default)]
pub struct ConnectionPoolStats {
    total_connection: AtomicU64,
    alive_connection: AtomicIsize,
}

impl ConnectionPoolStats {
    pub fn add_connection(self: &Arc<Self>) -> ConnectionPoolAliveConnectionGuard {
        self.total_connection.fetch_add(1, Ordering::Relaxed);
        self.alive_connection.fetch_add(1, Ordering::Relaxed);
        ConnectionPoolAliveConnectionGuard {
            stats: self.clone(),
        }
    }

    pub fn alive_count(&self) -> usize {
        self.alive_connection
            .load(Ordering::Relaxed)
            .try_into()
            .unwrap_or_default()
    }
}

pub struct ConnectionPoolAliveConnectionGuard {
    stats: Arc<ConnectionPoolStats>,
}

impl Drop for ConnectionPoolAliveConnectionGuard {
    fn drop(&mut self) {
        self.stats.alive_connection.fetch_sub(1, Ordering::Relaxed);
    }
}
