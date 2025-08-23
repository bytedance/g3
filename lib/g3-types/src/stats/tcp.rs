/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::cell::UnsafeCell;
use std::ops;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default, Clone, Copy)]
pub struct TcpIoSnapshot {
    pub in_bytes: u64,
    pub out_bytes: u64,
}

impl ops::Add for TcpIoSnapshot {
    type Output = TcpIoSnapshot;

    fn add(self, other: Self) -> Self {
        TcpIoSnapshot {
            in_bytes: self.in_bytes.wrapping_add(other.in_bytes),
            out_bytes: self.out_bytes.wrapping_add(other.out_bytes),
        }
    }
}

#[derive(Default)]
pub struct TcpIoStats {
    in_bytes: AtomicU64,
    out_bytes: AtomicU64,
}

impl TcpIoStats {
    pub fn add_in_bytes(&self, size: u64) {
        self.in_bytes.fetch_add(size, Ordering::Relaxed);
    }

    pub fn add_out_bytes(&self, size: u64) {
        self.out_bytes.fetch_add(size, Ordering::Relaxed);
    }

    pub fn get_in_bytes(&self) -> u64 {
        self.in_bytes.load(Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> TcpIoSnapshot {
        TcpIoSnapshot {
            in_bytes: self.in_bytes.load(Ordering::Relaxed),
            out_bytes: self.out_bytes.load(Ordering::Relaxed),
        }
    }
}

#[derive(Default)]
struct PerThreadTcpIoStats {
    in_bytes: UnsafeCell<u64>,
    out_bytes: UnsafeCell<u64>,
}

impl PerThreadTcpIoStats {
    impl_per_thread_unsafe_add_size!(add_in_bytes, in_bytes);
    impl_per_thread_unsafe_add_size!(add_out_bytes, out_bytes);

    impl_per_thread_unsafe_get!(get_in_bytes, in_bytes, u64);
    impl_per_thread_unsafe_get!(get_out_bytes, out_bytes, u64);

    fn snapshot(&self) -> TcpIoSnapshot {
        TcpIoSnapshot {
            in_bytes: self.get_in_bytes(),
            out_bytes: self.get_out_bytes(),
        }
    }
}

pub struct ThreadedTcpIoStats {
    a: TcpIoStats,
    p: Vec<PerThreadTcpIoStats>,
}

impl ThreadedTcpIoStats {
    pub fn new(thread_count: usize) -> Self {
        let mut p = Vec::with_capacity(thread_count);
        for _ in 0..thread_count {
            p.push(PerThreadTcpIoStats::default());
        }
        ThreadedTcpIoStats {
            a: TcpIoStats::default(),
            p,
        }
    }

    pub fn add_in_bytes(&self, tid: Option<usize>, size: u64) {
        if let Some(tid) = tid
            && let Some(s) = self.p.get(tid)
        {
            s.add_in_bytes(size);
            return;
        }
        self.a.add_in_bytes(size);
    }

    pub fn get_in_bytes(&self) -> u64 {
        self.p
            .iter()
            .map(|x| x.get_in_bytes())
            .fold(self.a.get_in_bytes(), |acc, x| acc + x)
    }

    pub fn add_out_bytes(&self, tid: Option<usize>, size: u64) {
        if let Some(tid) = tid
            && let Some(s) = self.p.get(tid)
        {
            s.add_out_bytes(size);
            return;
        }
        self.a.add_out_bytes(size);
    }

    pub fn snapshot(&self) -> TcpIoSnapshot {
        self.p
            .iter()
            .fold(self.a.snapshot(), |acc, x| acc + x.snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tcp_io_snapshot_default() {
        let snapshot = TcpIoSnapshot::default();
        assert_eq!(snapshot.in_bytes, 0);
        assert_eq!(snapshot.out_bytes, 0);
    }

    #[test]
    fn tcp_io_snapshot_clone_copy() {
        let snapshot1 = TcpIoSnapshot {
            in_bytes: 10,
            out_bytes: 20,
        };
        let snapshot2 = snapshot1;
        assert_eq!(snapshot2.in_bytes, 10);
        assert_eq!(snapshot2.out_bytes, 20);
    }

    #[test]
    fn tcp_io_snapshot_add() {
        let snapshot1 = TcpIoSnapshot {
            in_bytes: u64::MAX,
            out_bytes: 1,
        };
        let snapshot2 = TcpIoSnapshot {
            in_bytes: 1,
            out_bytes: 2,
        };
        let result = snapshot1 + snapshot2;
        assert_eq!(result.in_bytes, 0);
        assert_eq!(result.out_bytes, 3);
    }

    #[test]
    fn tcp_io_stats_default() {
        let stats = TcpIoStats::default();
        assert_eq!(stats.get_in_bytes(), 0);
        assert_eq!(stats.snapshot().out_bytes, 0);
    }

    #[test]
    fn tcp_io_stats_add_and_get() {
        let stats = TcpIoStats::default();
        stats.add_in_bytes(100);
        stats.add_out_bytes(200);
        assert_eq!(stats.get_in_bytes(), 100);
        assert_eq!(stats.snapshot().in_bytes, 100);
        assert_eq!(stats.snapshot().out_bytes, 200);
    }

    #[test]
    fn tcp_io_stats_snapshot() {
        let stats = TcpIoStats::default();
        stats.add_in_bytes(50);
        let snap = stats.snapshot();
        assert_eq!(snap.in_bytes, 50);
        assert_eq!(snap.out_bytes, 0);
    }

    #[test]
    fn per_thread_tcp_io_stats_default() {
        let per_thread = PerThreadTcpIoStats::default();
        assert_eq!(per_thread.get_in_bytes(), 0);
        assert_eq!(per_thread.get_out_bytes(), 0);
    }

    #[test]
    fn per_thread_tcp_io_stats_add_and_get() {
        let per_thread = PerThreadTcpIoStats::default();
        per_thread.add_in_bytes(300);
        per_thread.add_out_bytes(400);
        assert_eq!(per_thread.get_in_bytes(), 300);
        assert_eq!(per_thread.get_out_bytes(), 400);
    }

    #[test]
    fn per_thread_tcp_io_stats_snapshot() {
        let per_thread = PerThreadTcpIoStats::default();
        per_thread.add_in_bytes(500);
        let snap = per_thread.snapshot();
        assert_eq!(snap.in_bytes, 500);
        assert_eq!(snap.out_bytes, 0);
    }

    #[test]
    fn threaded_tcp_io_stats_new() {
        let threaded = ThreadedTcpIoStats::new(3);
        assert_eq!(threaded.p.len(), 3);
        assert_eq!(threaded.get_in_bytes(), 0);
    }

    #[test]
    fn threaded_tcp_io_stats_add_with_tid() {
        let threaded = ThreadedTcpIoStats::new(2);
        threaded.add_in_bytes(Some(0), 100);
        threaded.add_out_bytes(Some(1), 200);
        assert_eq!(threaded.get_in_bytes(), 100);
        assert_eq!(threaded.snapshot().out_bytes, 200);
    }

    #[test]
    fn threaded_tcp_io_stats_add_without_tid() {
        let threaded = ThreadedTcpIoStats::new(1);
        threaded.add_in_bytes(None, 300);
        threaded.add_out_bytes(None, 400);
        assert_eq!(threaded.get_in_bytes(), 300);
        assert_eq!(threaded.snapshot().out_bytes, 400);
    }

    #[test]
    fn threaded_tcp_io_stats_add_invalid_tid() {
        let threaded = ThreadedTcpIoStats::new(1);
        threaded.add_in_bytes(Some(5), 500);
        threaded.add_out_bytes(Some(10), 600);
        assert_eq!(threaded.get_in_bytes(), 500);
        assert_eq!(threaded.snapshot().out_bytes, 600);
    }

    #[test]
    fn threaded_tcp_io_stats_get_and_snapshot() {
        let threaded = ThreadedTcpIoStats::new(2);
        threaded.add_in_bytes(Some(0), 100);
        threaded.add_out_bytes(Some(1), 200);
        threaded.add_in_bytes(None, 300);
        threaded.add_out_bytes(None, 400);
        assert_eq!(threaded.get_in_bytes(), 400);
        let snap = threaded.snapshot();
        assert_eq!(snap.in_bytes, 400);
        assert_eq!(snap.out_bytes, 600);
    }

    #[test]
    fn threaded_tcp_io_stats_zero_threads() {
        let threaded = ThreadedTcpIoStats::new(0);
        threaded.add_in_bytes(Some(0), 100);
        threaded.add_out_bytes(None, 200);
        assert_eq!(threaded.get_in_bytes(), 100);
        assert_eq!(threaded.snapshot().out_bytes, 200);
    }

    #[test]
    fn threaded_tcp_io_stats_overflow() {
        let threaded = ThreadedTcpIoStats::new(1);
        threaded.add_in_bytes(Some(0), u64::MAX);
        threaded.add_in_bytes(None, 1);
        assert_eq!(threaded.snapshot().in_bytes, 0);
    }
}
