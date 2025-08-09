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
