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
pub(crate) struct FrontendStats {
    request_total: AtomicU64,
    request_invalid: AtomicU64,
    response_total: AtomicU64,
    response_fail: AtomicU64,
}

macro_rules! impl_for_field {
    ($add:ident, $take:ident, $field:ident) => {
        pub(crate) fn $add(&self) {
            self.$field.fetch_add(1, Ordering::Relaxed);
        }

        pub(crate) fn $take(&self) -> u64 {
            self.$field.swap(0, Ordering::Relaxed)
        }
    };
}

impl FrontendStats {
    impl_for_field!(add_request_total, take_request_total, request_total);
    impl_for_field!(add_request_invalid, take_request_invalid, request_invalid);
    impl_for_field!(add_response_total, take_response_total, response_total);
    impl_for_field!(add_response_fail, take_response_fail, response_fail);
}
