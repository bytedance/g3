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

#[derive(Clone, Copy, Default)]
struct PerThreadTcpIoStats {
    in_bytes: u64,
    out_bytes: u64,
}

impl PerThreadTcpIoStats {
    fn add_in_bytes(&self, size: u64) {
        unsafe {
            let r = &self.in_bytes as *const u64 as *mut u64;
            *r += size;
        }
    }

    fn add_out_bytes(&self, size: u64) {
        unsafe {
            let r = &self.out_bytes as *const u64 as *mut u64;
            *r += size;
        }
    }

    fn snapshot(&self) -> TcpIoSnapshot {
        TcpIoSnapshot {
            in_bytes: self.in_bytes,
            out_bytes: self.out_bytes,
        }
    }
}

pub struct ThreadedTcpIoStats {
    a: TcpIoStats,
    p: Vec<PerThreadTcpIoStats>,
}

impl ThreadedTcpIoStats {
    pub fn new(thread_count: usize) -> Self {
        ThreadedTcpIoStats {
            a: TcpIoStats::default(),
            p: vec![PerThreadTcpIoStats::default(); thread_count],
        }
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

    pub fn get_in_bytes(&self) -> u64 {
        self.p
            .iter()
            .map(|x| x.in_bytes)
            .fold(self.a.get_in_bytes(), |acc, x| acc + x)
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

    pub fn snapshot(&self) -> TcpIoSnapshot {
        self.p
            .iter()
            .fold(self.a.snapshot(), |acc, x| acc + x.snapshot())
    }
}
