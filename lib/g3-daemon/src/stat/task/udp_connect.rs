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

#[derive(Default)]
pub struct UdpConnectHalfConnectionStats {
    bytes: UnsafeCell<u64>,
    packets: UnsafeCell<u64>,
}

unsafe impl Sync for UdpConnectHalfConnectionStats {}

impl UdpConnectHalfConnectionStats {
    pub fn get_bytes(&self) -> u64 {
        let r = unsafe { &*self.bytes.get() };
        *r
    }

    pub fn get_packets(&self) -> u64 {
        let r = unsafe { &*self.packets.get() };
        *r
    }

    pub fn add_bytes(&self, size: u64) {
        let r = unsafe { &mut *self.bytes.get() };
        *r += size;
    }

    pub fn add_packet(&self) {
        self.add_packets(1);
    }

    pub fn add_packets(&self, n: usize) {
        let r = unsafe { &mut *self.packets.get() };
        *r += n as u64;
    }
}

#[derive(Default)]
pub struct UdpConnectConnectionStats {
    pub recv: UdpConnectHalfConnectionStats,
    pub send: UdpConnectHalfConnectionStats,
}
