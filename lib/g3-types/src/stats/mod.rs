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

#[cfg(test)]
mod tests {
    use std::cell::UnsafeCell;

    /// Test structure for impl_per_thread_unsafe_add_size
    struct SizeTest {
        counter: UnsafeCell<u64>,
    }
    impl SizeTest {
        impl_per_thread_unsafe_add_size!(add_size, counter);
    }

    /// Test structure for impl_per_thread_unsafe_add_packet
    struct PacketTest {
        count: UnsafeCell<u64>,
    }
    impl PacketTest {
        impl_per_thread_unsafe_add_packet!(add_packet, count);
    }

    /// Test structure for impl_per_thread_unsafe_get
    struct GetTest {
        value: UnsafeCell<u64>,
    }
    impl GetTest {
        impl_per_thread_unsafe_get!(get_value, value, u64);
    }

    #[test]
    fn add_size_macro_basic() {
        let target = SizeTest {
            counter: UnsafeCell::new(0),
        };
        target.add_size(100);
        unsafe {
            assert_eq!(*target.counter.get(), 100);
        }
    }

    #[test]
    fn add_size_macro_sequential() {
        let target = SizeTest {
            counter: UnsafeCell::new(10),
        };
        target.add_size(20);
        target.add_size(30);
        unsafe {
            assert_eq!(*target.counter.get(), 60);
        }
    }

    #[test]
    fn add_size_macro_near_boundary() {
        let target = SizeTest {
            counter: UnsafeCell::new(u64::MAX - 10),
        };
        target.add_size(5);
        unsafe {
            assert_eq!(*target.counter.get(), u64::MAX - 5);
        }

        target.add_size(5);
        unsafe {
            assert_eq!(*target.counter.get(), u64::MAX);
        }
    }

    #[test]
    fn add_packet_macro_basic() {
        let target = PacketTest {
            count: UnsafeCell::new(0),
        };
        target.add_packet();
        target.add_packet();
        unsafe {
            assert_eq!(*target.count.get(), 2);
        }
    }

    #[test]
    fn get_macro_basic() {
        let target = GetTest {
            value: UnsafeCell::new(42),
        };
        assert_eq!(target.get_value(), 42);
    }
}
