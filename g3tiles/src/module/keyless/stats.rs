/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) struct KeylessRelayStats {
    req_total: AtomicU64,
    req_pass: AtomicU64,
    req_fail: AtomicU64,
    rsp_drop: AtomicU64,
    rsp_pass: AtomicU64,
    rsp_fail: AtomicU64,
}

impl Default for KeylessRelayStats {
    fn default() -> Self {
        KeylessRelayStats {
            req_total: AtomicU64::new(0),
            req_pass: AtomicU64::new(0),
            req_fail: AtomicU64::new(0),
            rsp_drop: AtomicU64::new(0),
            rsp_pass: AtomicU64::new(0),
            rsp_fail: AtomicU64::new(0),
        }
    }
}

macro_rules! impl_field {
    ($field:ident, $add:ident) => {
        pub(crate) fn $add(&self) {
            self.$field.fetch_add(1, Ordering::Relaxed);
        }
    };
}

impl KeylessRelayStats {
    impl_field!(req_total, add_req_total);
    impl_field!(req_pass, add_req_pass);
    impl_field!(req_fail, add_req_fail);
    impl_field!(rsp_drop, add_rsp_drop);
    impl_field!(rsp_pass, add_rsp_pass);
    impl_field!(rsp_fail, add_rsp_fail);

    pub(crate) fn snapshot(&self) -> KeylessRelaySnapshot {
        KeylessRelaySnapshot {
            req_total: self.req_total.load(Ordering::Relaxed),
            req_pass: self.req_pass.load(Ordering::Relaxed),
            req_fail: self.req_fail.load(Ordering::Relaxed),
            rsp_drop: self.rsp_drop.load(Ordering::Relaxed),
            rsp_pass: self.rsp_pass.load(Ordering::Relaxed),
            rsp_fail: self.rsp_fail.load(Ordering::Relaxed),
        }
    }
}

pub(crate) struct KeylessRelaySnapshot {
    pub(crate) req_total: u64,
    pub(crate) req_pass: u64,
    pub(crate) req_fail: u64,
    pub(crate) rsp_drop: u64,
    pub(crate) rsp_pass: u64,
    pub(crate) rsp_fail: u64,
}
