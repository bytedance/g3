/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::cell::UnsafeCell;
use std::ops;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default, Clone, Copy)]
pub struct UdpIoSnapshot {
    pub in_packets: u64,
    pub in_bytes: u64,
    pub out_packets: u64,
    pub out_bytes: u64,
}

impl ops::Add for UdpIoSnapshot {
    type Output = UdpIoSnapshot;

    fn add(self, other: Self) -> Self {
        UdpIoSnapshot {
            in_packets: self.in_packets.wrapping_add(other.in_packets),
            in_bytes: self.in_bytes.wrapping_add(other.in_bytes),
            out_packets: self.out_packets.wrapping_add(other.out_packets),
            out_bytes: self.out_bytes.wrapping_add(other.out_bytes),
        }
    }
}

#[derive(Default)]
pub struct UdpIoStats {
    in_packets: AtomicU64,
    in_bytes: AtomicU64,
    out_packets: AtomicU64,
    out_bytes: AtomicU64,
}

impl UdpIoStats {
    pub fn add_in_packet(&self) {
        self.add_in_packets(1);
    }

    pub fn add_in_packets(&self, n: usize) {
        self.in_packets.fetch_add(n as u64, Ordering::Relaxed);
    }

    pub fn add_in_bytes(&self, size: u64) {
        self.in_bytes.fetch_add(size, Ordering::Relaxed);
    }

    pub fn add_out_packet(&self) {
        self.add_out_packets(1);
    }

    pub fn add_out_packets(&self, n: usize) {
        self.out_packets.fetch_add(n as u64, Ordering::Relaxed);
    }

    pub fn add_out_bytes(&self, size: u64) {
        self.out_bytes.fetch_add(size, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> UdpIoSnapshot {
        UdpIoSnapshot {
            in_packets: self.in_packets.load(Ordering::Relaxed),
            in_bytes: self.in_bytes.load(Ordering::Relaxed),
            out_packets: self.out_packets.load(Ordering::Relaxed),
            out_bytes: self.out_bytes.load(Ordering::Relaxed),
        }
    }
}

#[derive(Default)]
struct PerThreadUdpIoStats {
    in_packets: UnsafeCell<u64>,
    in_bytes: UnsafeCell<u64>,
    out_packets: UnsafeCell<u64>,
    out_bytes: UnsafeCell<u64>,
}

impl PerThreadUdpIoStats {
    impl_per_thread_unsafe_add_size!(add_in_bytes, in_bytes);
    impl_per_thread_unsafe_add_packet!(add_in_packet, in_packets);
    impl_per_thread_unsafe_add_size!(add_out_bytes, out_bytes);
    impl_per_thread_unsafe_add_packet!(add_out_packet, out_packets);

    impl_per_thread_unsafe_get!(get_in_bytes, in_bytes, u64);
    impl_per_thread_unsafe_get!(get_in_packets, in_packets, u64);
    impl_per_thread_unsafe_get!(get_out_bytes, out_bytes, u64);
    impl_per_thread_unsafe_get!(get_out_packets, out_packets, u64);

    fn snapshot(&self) -> UdpIoSnapshot {
        UdpIoSnapshot {
            in_packets: self.get_in_packets(),
            in_bytes: self.get_in_bytes(),
            out_packets: self.get_out_packets(),
            out_bytes: self.get_out_bytes(),
        }
    }
}

pub struct ThreadedUdpIoStats {
    a: UdpIoStats,
    p: Vec<PerThreadUdpIoStats>,
}

impl ThreadedUdpIoStats {
    pub fn new(thread_count: usize) -> Self {
        let mut p = Vec::with_capacity(thread_count);
        for _ in 0..thread_count {
            p.push(PerThreadUdpIoStats::default());
        }
        ThreadedUdpIoStats {
            a: UdpIoStats::default(),
            p,
        }
    }

    pub fn add_in_packet(&self, tid: Option<usize>) {
        if let Some(tid) = tid
            && let Some(s) = self.p.get(tid)
        {
            s.add_in_packet();
            return;
        }
        self.a.add_in_packet();
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

    pub fn add_out_packet(&self, tid: Option<usize>) {
        if let Some(tid) = tid
            && let Some(s) = self.p.get(tid)
        {
            s.add_out_packet();
            return;
        }
        self.a.add_out_packet();
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

    pub fn snapshot(&self) -> UdpIoSnapshot {
        self.p
            .iter()
            .fold(self.a.snapshot(), |acc, x| acc + x.snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn udp_io_snapshot_default() {
        let snapshot = UdpIoSnapshot::default();
        assert_eq!(snapshot.in_packets, 0);
        assert_eq!(snapshot.in_bytes, 0);
        assert_eq!(snapshot.out_packets, 0);
        assert_eq!(snapshot.out_bytes, 0);
    }

    #[test]
    fn udp_io_snapshot_clone_copy() {
        let snapshot1 = UdpIoSnapshot {
            in_packets: 10,
            in_bytes: 20,
            out_packets: 30,
            out_bytes: 40,
        };
        let snapshot2 = snapshot1;
        assert_eq!(snapshot2.in_packets, 10);
        assert_eq!(snapshot2.in_bytes, 20);
        assert_eq!(snapshot2.out_packets, 30);
        assert_eq!(snapshot2.out_bytes, 40);
    }

    #[test]
    fn udp_io_snapshot_add() {
        let snapshot1 = UdpIoSnapshot {
            in_packets: u64::MAX,
            in_bytes: 1,
            out_packets: 2,
            out_bytes: 3,
        };
        let snapshot2 = UdpIoSnapshot {
            in_packets: 1,
            in_bytes: 4,
            out_packets: 5,
            out_bytes: 6,
        };
        let result = snapshot1 + snapshot2;
        assert_eq!(result.in_packets, 0);
        assert_eq!(result.in_bytes, 5);
        assert_eq!(result.out_packets, 7);
        assert_eq!(result.out_bytes, 9);
    }

    #[test]
    fn udp_io_stats_default() {
        let stats = UdpIoStats::default();
        assert_eq!(stats.snapshot().in_packets, 0);
        assert_eq!(stats.snapshot().out_bytes, 0);
    }

    #[test]
    fn udp_io_stats_add_and_snapshot() {
        let stats = UdpIoStats::default();
        stats.add_in_packet();
        stats.add_in_packets(2);
        stats.add_in_bytes(100);
        stats.add_out_packet();
        stats.add_out_packets(3);
        stats.add_out_bytes(200);
        let snap = stats.snapshot();
        assert_eq!(snap.in_packets, 3);
        assert_eq!(snap.in_bytes, 100);
        assert_eq!(snap.out_packets, 4);
        assert_eq!(snap.out_bytes, 200);
    }

    #[test]
    fn per_thread_udp_io_stats_default() {
        let per_thread = PerThreadUdpIoStats::default();
        assert_eq!(per_thread.get_in_packets(), 0);
        assert_eq!(per_thread.get_out_bytes(), 0);
    }

    #[test]
    fn per_thread_udp_io_stats_add_and_get() {
        let per_thread = PerThreadUdpIoStats::default();
        per_thread.add_in_packet();
        per_thread.add_in_bytes(300);
        per_thread.add_out_packet();
        per_thread.add_out_bytes(400);
        assert_eq!(per_thread.get_in_packets(), 1);
        assert_eq!(per_thread.get_in_bytes(), 300);
        assert_eq!(per_thread.get_out_packets(), 1);
        assert_eq!(per_thread.get_out_bytes(), 400);
    }

    #[test]
    fn per_thread_udp_io_stats_snapshot() {
        let per_thread = PerThreadUdpIoStats::default();
        per_thread.add_in_packet();
        per_thread.add_out_bytes(500);
        let snap = per_thread.snapshot();
        assert_eq!(snap.in_packets, 1);
        assert_eq!(snap.in_bytes, 0);
        assert_eq!(snap.out_packets, 0);
        assert_eq!(snap.out_bytes, 500);
    }

    #[test]
    fn threaded_udp_io_stats_new() {
        let threaded = ThreadedUdpIoStats::new(4);
        assert_eq!(threaded.p.len(), 4);
        assert_eq!(threaded.snapshot().in_packets, 0);
    }

    #[test]
    fn threaded_udp_io_stats_add_with_tid() {
        let threaded = ThreadedUdpIoStats::new(2);
        threaded.add_in_packet(Some(0));
        threaded.add_in_bytes(Some(0), 100);
        threaded.add_out_packet(Some(1));
        threaded.add_out_bytes(Some(1), 200);
        let snap = threaded.snapshot();
        assert_eq!(snap.in_packets, 1);
        assert_eq!(snap.in_bytes, 100);
        assert_eq!(snap.out_packets, 1);
        assert_eq!(snap.out_bytes, 200);
    }

    #[test]
    fn threaded_udp_io_stats_add_without_tid() {
        let threaded = ThreadedUdpIoStats::new(1);
        threaded.add_in_packet(None);
        threaded.add_in_bytes(None, 300);
        threaded.add_out_packet(None);
        threaded.add_out_bytes(None, 400);
        let snap = threaded.snapshot();
        assert_eq!(snap.in_packets, 1);
        assert_eq!(snap.in_bytes, 300);
        assert_eq!(snap.out_packets, 1);
        assert_eq!(snap.out_bytes, 400);
    }

    #[test]
    fn threaded_udp_io_stats_add_invalid_tid() {
        let threaded = ThreadedUdpIoStats::new(1);
        threaded.add_in_packet(Some(5));
        threaded.add_in_bytes(Some(10), 500);
        threaded.add_out_packet(Some(15));
        threaded.add_out_bytes(Some(20), 600);
        let snap = threaded.snapshot();
        assert_eq!(snap.in_packets, 1);
        assert_eq!(snap.in_bytes, 500);
        assert_eq!(snap.out_packets, 1);
        assert_eq!(snap.out_bytes, 600);
    }

    #[test]
    fn threaded_udp_io_stats_snapshot() {
        let threaded = ThreadedUdpIoStats::new(2);
        threaded.add_in_packet(Some(0));
        threaded.add_in_bytes(Some(0), 100);
        threaded.add_out_packet(Some(1));
        threaded.add_out_bytes(Some(1), 200);
        threaded.add_in_packet(None);
        threaded.add_in_bytes(None, 300);
        threaded.add_out_packet(None);
        threaded.add_out_bytes(None, 400);
        let snap = threaded.snapshot();
        assert_eq!(snap.in_packets, 2);
        assert_eq!(snap.in_bytes, 400);
        assert_eq!(snap.out_packets, 2);
        assert_eq!(snap.out_bytes, 600);
    }

    #[test]
    fn threaded_udp_io_stats_zero_threads() {
        let threaded = ThreadedUdpIoStats::new(0);
        threaded.add_in_packet(Some(0));
        threaded.add_in_bytes(None, 100);
        threaded.add_out_packet(None);
        threaded.add_out_bytes(Some(1), 200);
        let snap = threaded.snapshot();
        assert_eq!(snap.in_packets, 1);
        assert_eq!(snap.in_bytes, 100);
        assert_eq!(snap.out_packets, 1);
        assert_eq!(snap.out_bytes, 200);
    }

    #[test]
    fn threaded_udp_io_stats_overflow() {
        let threaded = ThreadedUdpIoStats::new(1);
        threaded.add_in_bytes(Some(0), u64::MAX);
        threaded.add_in_bytes(None, 1);
        assert_eq!(threaded.snapshot().in_bytes, 0);
    }
}
