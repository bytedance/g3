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
}

#[derive(Clone, Default)]
pub struct TcpStreamConnectionStats {
    pub read: TcpStreamHalfConnectionStats,
    pub write: TcpStreamHalfConnectionStats,
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
