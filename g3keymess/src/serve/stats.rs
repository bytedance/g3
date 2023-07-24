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

use std::sync::atomic::{AtomicI32, AtomicIsize, AtomicU64, Ordering};
use std::sync::Arc;

use arc_swap::ArcSwapOption;

use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_types::stats::StatId;

use crate::protocol::KeylessResponseErrorCode;

#[derive(Default)]
pub(crate) struct KeyServerRequestStats {
    total: AtomicU64,
    alive_count: AtomicI32,

    passed: AtomicU64,
    key_not_found: AtomicU64,
    crypto_fail: AtomicU64,
    bad_op_code: AtomicU64,
    format_error: AtomicU64,
    other_fail: AtomicU64,
}

#[derive(Default)]
pub(crate) struct KeyServerRequestSnapshot {
    pub(crate) total: u64,
    pub(crate) alive_count: i32,

    pub(crate) passed: u64,
    pub(crate) key_not_found: u64,
    pub(crate) crypto_fail: u64,
    pub(crate) bad_op_code: u64,
    pub(crate) format_error: u64,
    pub(crate) other_fail: u64,
}

impl KeyServerRequestStats {
    pub(crate) fn add_total(&self) {
        self.total.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn inc_alive(&self) {
        self.alive_count.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn dec_alive(&self) {
        self.alive_count.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn add_passed(&self) {
        self.passed.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_key_not_found(&self) {
        self.key_not_found.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_crypto_fail(&self) {
        self.crypto_fail.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_bad_op_code(&self) {
        self.bad_op_code.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_format_error(&self) {
        self.format_error.fetch_add(1, Ordering::Relaxed);
    }

    fn add_other_fail(&self) {
        self.other_fail.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_by_error_code(&self, code: KeylessResponseErrorCode) {
        match code {
            KeylessResponseErrorCode::NoError => self.add_passed(),
            KeylessResponseErrorCode::KeyNotFound => self.add_key_not_found(),
            KeylessResponseErrorCode::CryptographyFailure => self.add_crypto_fail(),
            KeylessResponseErrorCode::BadOpCode => self.add_bad_op_code(),
            KeylessResponseErrorCode::FormatError => self.add_format_error(),
            _ => self.add_other_fail(),
        }
    }

    pub(crate) fn snapshot(&self) -> KeyServerRequestSnapshot {
        KeyServerRequestSnapshot {
            total: self.total.load(Ordering::Relaxed),
            alive_count: self.alive_count.load(Ordering::Relaxed),
            passed: self.passed.load(Ordering::Relaxed),
            key_not_found: self.key_not_found.load(Ordering::Relaxed),
            crypto_fail: self.crypto_fail.load(Ordering::Relaxed),
            bad_op_code: self.bad_op_code.load(Ordering::Relaxed),
            format_error: self.format_error.load(Ordering::Relaxed),
            other_fail: self.other_fail.load(Ordering::Relaxed),
        }
    }
}

pub(crate) struct KeyServerStats {
    name: MetricsName,
    id: StatId,

    extra_metrics_tags: Arc<ArcSwapOption<StaticMetricsTags>>,

    online: AtomicIsize,

    task_total: AtomicU64,
    task_alive_count: AtomicI32,

    pub(crate) ping_pong: Arc<KeyServerRequestStats>,
    pub(crate) rsa_decrypt: Arc<KeyServerRequestStats>,
    pub(crate) rsa_sign: Arc<KeyServerRequestStats>,
    pub(crate) rsa_pss_sign: Arc<KeyServerRequestStats>,
    pub(crate) ecdsa_sign: Arc<KeyServerRequestStats>,
    pub(crate) ed25519_sign: Arc<KeyServerRequestStats>,
    pub(crate) other: Arc<KeyServerRequestStats>,
}

#[derive(Default)]
pub(crate) struct KeyServerSnapshot {
    pub(crate) task_total: u64,

    pub(crate) ping_pong: KeyServerRequestSnapshot,
    pub(crate) rsa_decrypt: KeyServerRequestSnapshot,
    pub(crate) rsa_sign: KeyServerRequestSnapshot,
    pub(crate) rsa_pss_sign: KeyServerRequestSnapshot,
    pub(crate) ecdsa_sign: KeyServerRequestSnapshot,
    pub(crate) ed25519_sign: KeyServerRequestSnapshot,
}

impl KeyServerStats {
    pub(crate) fn new(name: &MetricsName) -> Self {
        KeyServerStats {
            name: name.clone(),
            id: StatId::new(),
            extra_metrics_tags: Arc::new(ArcSwapOption::new(None)),
            online: AtomicIsize::new(0),
            task_total: AtomicU64::new(0),
            task_alive_count: AtomicI32::new(0),
            ping_pong: Arc::new(KeyServerRequestStats::default()),
            rsa_decrypt: Arc::new(KeyServerRequestStats::default()),
            rsa_sign: Arc::new(KeyServerRequestStats::default()),
            rsa_pss_sign: Arc::new(KeyServerRequestStats::default()),
            ecdsa_sign: Arc::new(KeyServerRequestStats::default()),
            ed25519_sign: Arc::new(KeyServerRequestStats::default()),
            other: Arc::new(KeyServerRequestStats::default()),
        }
    }

    #[inline]
    pub(crate) fn name(&self) -> &MetricsName {
        &self.name
    }

    #[inline]
    pub(crate) fn stat_id(&self) -> StatId {
        self.id
    }

    pub(crate) fn set_online(&self) {
        self.online.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn set_offline(&self) {
        self.online.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn set_extra_tags(&self, tags: Option<Arc<StaticMetricsTags>>) {
        self.extra_metrics_tags.store(tags);
    }

    pub(crate) fn extra_tags(&self) -> &Arc<ArcSwapOption<StaticMetricsTags>> {
        &self.extra_metrics_tags
    }

    pub(crate) fn add_task(&self) {
        self.task_total.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn inc_alive_task(&self) {
        self.task_alive_count.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn dec_alive_task(&self) {
        self.task_alive_count.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn is_online(&self) -> bool {
        self.online.load(Ordering::Relaxed) > 0
    }

    pub(crate) fn get_task_total(&self) -> u64 {
        self.task_total.load(Ordering::Relaxed)
    }

    pub(crate) fn get_alive_count(&self) -> i32 {
        self.task_alive_count.load(Ordering::Relaxed)
    }
}
