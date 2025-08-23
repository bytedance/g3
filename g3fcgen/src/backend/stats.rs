/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub(crate) struct BackendStats {
    refresh_total: AtomicU64,
    refresh_ok: AtomicU64,
    request_total: AtomicU64,
    request_ok: AtomicU64,
}

macro_rules! impl_for_field {
    ($add:ident, $take:ident, $field:ident) => {
        pub(super) fn $add(&self) {
            self.$field.fetch_add(1, Ordering::Relaxed);
        }

        pub(crate) fn $take(&self) -> u64 {
            self.$field.swap(0, Ordering::Relaxed)
        }
    };
}

impl BackendStats {
    impl_for_field!(add_refresh_total, take_refresh_total, refresh_total);
    impl_for_field!(add_refresh_ok, take_refresh_ok, refresh_ok);
    impl_for_field!(add_request_total, take_request_total, request_total);
    impl_for_field!(add_request_ok, take_request_ok, request_ok);
}
