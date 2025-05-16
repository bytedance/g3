/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::cell::UnsafeCell;

use crate::stat::remote::TcpConnectionTaskRemoteStats;

#[derive(Default)]
pub struct TcpStreamHalfConnectionStats {
    bytes: UnsafeCell<u64>,
}

unsafe impl Sync for TcpStreamHalfConnectionStats {}

impl Clone for TcpStreamHalfConnectionStats {
    fn clone(&self) -> Self {
        TcpStreamHalfConnectionStats {
            bytes: UnsafeCell::new(self.get_bytes()),
        }
    }
}

impl TcpStreamHalfConnectionStats {
    pub fn get_bytes(&self) -> u64 {
        let r = unsafe { &*self.bytes.get() };
        *r
    }

    pub fn add_bytes(&self, size: u64) {
        let r = unsafe { &mut *self.bytes.get() };
        *r += size;
    }

    pub fn reset(&self) {
        let r = unsafe { &mut *self.bytes.get() };
        *r = 0;
    }
}

#[derive(Clone, Default)]
pub struct TcpStreamConnectionStats {
    pub read: TcpStreamHalfConnectionStats,
    pub write: TcpStreamHalfConnectionStats,
}

impl TcpStreamConnectionStats {
    pub fn reset(&self) {
        self.read.reset();
        self.write.reset();
    }
}

#[derive(Default)]
pub struct TcpStreamTaskStats {
    pub clt: TcpStreamConnectionStats,
    pub ups: TcpStreamConnectionStats,
}

impl TcpStreamTaskStats {
    pub fn with_clt_stats(clt: TcpStreamConnectionStats) -> Self {
        TcpStreamTaskStats {
            clt,
            ups: TcpStreamConnectionStats::default(),
        }
    }
}

impl TcpConnectionTaskRemoteStats for TcpStreamTaskStats {
    fn add_read_bytes(&self, size: u64) {
        self.ups.read.add_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.ups.write.add_bytes(size);
    }
}
