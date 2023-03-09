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

use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};

#[derive(Default)]
pub(crate) struct ConnectionStats {
    http: AtomicU64,
    socks: AtomicU64,
}

#[derive(Default)]
pub(crate) struct ConnectionSnapshot {
    pub(crate) http: u64,
    pub(crate) socks: u64,
}

impl ConnectionStats {
    pub(crate) fn add_http(&self) {
        self.http.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn get_http(&self) -> u64 {
        self.http.load(Ordering::Relaxed)
    }

    pub(crate) fn add_socks(&self) {
        self.socks.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn get_socks(&self) -> u64 {
        self.socks.load(Ordering::Relaxed)
    }
}

#[derive(Default)]
pub(crate) struct L7ConnectionAliveStats {
    http: AtomicI32,
}

impl L7ConnectionAliveStats {
    pub(crate) fn inc_http(&self) {
        self.http.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn dec_http(&self) {
        self.http.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn get_http(&self) -> i32 {
        self.http.load(Ordering::Relaxed)
    }
}
