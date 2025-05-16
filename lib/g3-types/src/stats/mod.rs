/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

macro_rules! impl_per_thread_unsafe_add_size {
    ($method:ident, $field:ident) => {
        fn $method(&self, size: u64) {
            let r = unsafe { &mut *self.$field.get() };
            *r += size;
        }
    };
}

macro_rules! impl_per_thread_unsafe_add_packet {
    ($method:ident, $field:ident) => {
        fn $method(&self) {
            let r = unsafe { &mut *self.$field.get() };
            *r += 1;
        }
    };
}

macro_rules! impl_per_thread_unsafe_get {
    ($method:ident, $field:ident, $r:ty) => {
        fn $method(&self) -> $r {
            let r = unsafe { &*self.$field.get() };
            *r
        }
    };
}

mod id;
pub use id::StatId;

mod tcp;
pub use tcp::{TcpIoSnapshot, TcpIoStats, ThreadedTcpIoStats};

mod udp;
pub use udp::{ThreadedUdpIoStats, UdpIoSnapshot, UdpIoStats};

mod pool;
pub use pool::ConnectionPoolStats;

mod map;
pub use map::GlobalStatsMap;
