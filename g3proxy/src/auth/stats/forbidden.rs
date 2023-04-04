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
use std::sync::Arc;

use arc_swap::ArcSwapOption;

use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_types::stats::StatId;

use crate::auth::UserType;

pub(crate) struct UserForbiddenStats {
    id: StatId,
    user_group: MetricsName,
    user: String,
    user_type: UserType,
    server: MetricsName,
    server_extra_tags: Arc<ArcSwapOption<StaticMetricsTags>>,
    auth_failed: AtomicU64,
    user_expired: AtomicU64,
    user_blocked: AtomicU64,
    fully_loaded: AtomicU64,
    rate_limited: AtomicU64,
    proto_banned: AtomicU64,
    dest_denied: AtomicU64,
    ip_blocked: AtomicU64,
    ua_blocked: AtomicU64,
    log_skipped: AtomicU64,
}

#[derive(Default)]
pub(crate) struct UserForbiddenSnapshot {
    pub(crate) auth_failed: u64,
    pub(crate) user_expired: u64,
    pub(crate) user_blocked: u64,
    pub(crate) fully_loaded: u64,
    pub(crate) rate_limited: u64,
    pub(crate) proto_banned: u64,
    pub(crate) dest_denied: u64,
    pub(crate) ip_blocked: u64,
    pub(crate) ua_blocked: u64,
    pub(crate) log_skipped: u64,
}

impl UserForbiddenStats {
    pub(crate) fn new(
        user_group: &MetricsName,
        user: &str,
        user_type: UserType,
        server: &MetricsName,
        server_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
    ) -> Self {
        UserForbiddenStats {
            id: StatId::new(),
            user_group: user_group.clone(),
            user: user.to_string(),
            user_type,
            server: server.clone(),
            server_extra_tags: Arc::clone(server_extra_tags),
            auth_failed: Default::default(),
            user_expired: Default::default(),
            user_blocked: Default::default(),
            fully_loaded: Default::default(),
            rate_limited: Default::default(),
            proto_banned: Default::default(),
            dest_denied: Default::default(),
            ip_blocked: Default::default(),
            ua_blocked: Default::default(),
            log_skipped: Default::default(),
        }
    }

    #[inline]
    pub(crate) fn stat_id(&self) -> StatId {
        self.id
    }

    #[inline]
    pub(crate) fn user_group(&self) -> &MetricsName {
        &self.user_group
    }

    #[inline]
    pub(crate) fn user(&self) -> &str {
        &self.user
    }

    #[inline]
    pub(crate) fn user_type(&self) -> &str {
        self.user_type.as_str()
    }

    #[inline]
    pub(crate) fn server(&self) -> &MetricsName {
        &self.server
    }

    pub(crate) fn server_extra_tags(&self) -> Option<Arc<StaticMetricsTags>> {
        let guard = self.server_extra_tags.load();
        (*guard).as_ref().map(Arc::clone)
    }

    pub(crate) fn snapshot(&self) -> UserForbiddenSnapshot {
        UserForbiddenSnapshot {
            auth_failed: self.auth_failed.load(Ordering::Relaxed),
            user_expired: self.user_expired.load(Ordering::Relaxed),
            user_blocked: self.user_blocked.load(Ordering::Relaxed),
            fully_loaded: self.fully_loaded.load(Ordering::Relaxed),
            rate_limited: self.rate_limited.load(Ordering::Relaxed),
            proto_banned: self.proto_banned.load(Ordering::Relaxed),
            dest_denied: self.dest_denied.load(Ordering::Relaxed),
            ip_blocked: self.ip_blocked.load(Ordering::Relaxed),
            ua_blocked: self.ua_blocked.load(Ordering::Relaxed),
            log_skipped: self.log_skipped.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn add_auth_failed(&self) {
        self.auth_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_user_expired(&self) {
        self.user_expired.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_user_blocked(&self) {
        self.user_blocked.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_fully_loaded(&self) {
        self.fully_loaded.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_rate_limited(&self) {
        self.rate_limited.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_proto_banned(&self) {
        self.proto_banned.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_dest_denied(&self) {
        self.dest_denied.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_ip_blocked(&self) {
        self.ip_blocked.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_ua_blocked(&self) {
        self.ua_blocked.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_log_skipped(&self) {
        self.log_skipped.fetch_add(1, Ordering::Relaxed);
    }
}
