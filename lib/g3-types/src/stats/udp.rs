/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
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
        if let Some(tid) = tid {
            if let Some(s) = self.p.get(tid) {
                s.add_in_packet();
                return;
            }
        }
        self.a.add_in_packet();
    }

    pub fn add_in_bytes(&self, tid: Option<usize>, size: u64) {
        if let Some(tid) = tid {
            if let Some(s) = self.p.get(tid) {
                s.add_in_bytes(size);
                return;
            }
        }
        self.a.add_in_bytes(size);
    }

    pub fn add_out_packet(&self, tid: Option<usize>) {
        if let Some(tid) = tid {
            if let Some(s) = self.p.get(tid) {
                s.add_out_packet();
                return;
            }
        }
        self.a.add_out_packet();
    }

    pub fn add_out_bytes(&self, tid: Option<usize>, size: u64) {
        if let Some(tid) = tid {
            if let Some(s) = self.p.get(tid) {
                s.add_out_bytes(size);
                return;
            }
        }
        self.a.add_out_bytes(size);
    }

    pub fn snapshot(&self) -> UdpIoSnapshot {
        self.p
            .iter()
            .fold(self.a.snapshot(), |acc, x| acc + x.snapshot())
    }
}
