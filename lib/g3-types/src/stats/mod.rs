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
