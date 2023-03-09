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
use std::sync::Arc;

use arc_swap::ArcSwapOption;

use g3_types::metrics::StaticMetricsTags;
use g3_types::stats::{StatId, TcpIoSnapshot, UdpIoSnapshot};

use crate::stat::types::UntrustedTaskStatsSnapshot;

pub(crate) trait ServerStats {
    fn name(&self) -> &str;
    fn stat_id(&self) -> StatId;
    fn extra_tags(&self) -> &Arc<ArcSwapOption<StaticMetricsTags>>;

    fn is_online(&self) -> bool;

    /// count for all connections
    fn get_conn_total(&self) -> u64;
    /// count for real tasks
    fn get_task_total(&self) -> u64;
    /// count for alive tasks
    fn get_alive_count(&self) -> i32;

    fn tcp_io_snapshot(&self) -> Option<TcpIoSnapshot> {
        None
    }
    fn udp_io_snapshot(&self) -> Option<UdpIoSnapshot> {
        None
    }
    fn forbidden_stats(&self) -> ServerForbiddenSnapshot;

    // for tasks that we should not trust them but must drain them
    fn untrusted_snapshot(&self) -> Option<UntrustedTaskStatsSnapshot> {
        None
    }
}

pub(crate) type ArcServerStats = Arc<dyn ServerStats + Send + Sync>;

#[derive(Default)]
pub(crate) struct ServerForbiddenSnapshot {
    pub(crate) auth_failed: u64,
    pub(crate) dest_denied: u64,
    pub(crate) user_blocked: u64,
}

#[derive(Default)]
pub(crate) struct ServerForbiddenStats {
    auth_failed: AtomicU64,
    dest_denied: AtomicU64,
    user_blocked: AtomicU64,
}

impl ServerForbiddenStats {
    pub(crate) fn add_auth_failed(&self) {
        self.auth_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_dest_denied(&self) {
        self.dest_denied.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_user_blocked(&self) {
        self.user_blocked.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn snapshot(&self) -> ServerForbiddenSnapshot {
        ServerForbiddenSnapshot {
            auth_failed: self.auth_failed.load(Ordering::Relaxed),
            dest_denied: self.dest_denied.load(Ordering::Relaxed),
            user_blocked: self.user_blocked.load(Ordering::Relaxed),
        }
    }
}

#[derive(Default)]
pub(crate) struct ServerPerTaskStats {
    task_total: AtomicU64,
    alive_count: AtomicI32,
}

impl ServerPerTaskStats {
    pub(super) fn add_task(&self) {
        self.task_total.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn inc_alive_task(&self) {
        self.alive_count.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn dec_alive_task(&self) {
        self.alive_count.fetch_sub(1, Ordering::Relaxed);
    }

    pub(super) fn get_task_total(&self) -> u64 {
        self.task_total.load(Ordering::Relaxed)
    }

    pub(super) fn get_alive_count(&self) -> i32 {
        self.alive_count.load(Ordering::Relaxed)
    }
}
