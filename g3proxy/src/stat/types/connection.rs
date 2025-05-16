/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
