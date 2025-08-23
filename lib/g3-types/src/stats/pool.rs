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

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn initial_state() {
        let stats = ConnectionPoolStats::default();
        assert_eq!(stats.alive_count(), 0);
    }

    #[test]
    fn single_connection_flow() {
        let stats = Arc::new(ConnectionPoolStats::default());
        assert_eq!(stats.alive_count(), 0);

        let guard = stats.add_connection();
        assert_eq!(stats.alive_count(), 1);

        drop(guard);
        assert_eq!(stats.alive_count(), 0);
    }

    #[test]
    fn concurrent_connections() {
        let stats = Arc::new(ConnectionPoolStats::default());
        let mut guards = Vec::new();

        for _ in 0..10 {
            guards.push(stats.add_connection());
        }
        assert_eq!(stats.alive_count(), 10);

        let handles: Vec<_> = guards
            .into_iter()
            .map(|guard| thread::spawn(move || drop(guard)))
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(stats.alive_count(), 0);
    }

    #[test]
    fn alive_count_conversion() {
        let stats = ConnectionPoolStats::default();

        stats.alive_connection.store(42, Ordering::Relaxed);
        assert_eq!(stats.alive_count(), 42);

        stats.alive_connection.store(-1, Ordering::Relaxed);
        assert_eq!(stats.alive_count(), 0);
    }
}
