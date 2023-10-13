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
