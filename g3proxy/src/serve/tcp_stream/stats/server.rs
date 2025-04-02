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

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicIsize, AtomicU64, Ordering};

use arc_swap::ArcSwapOption;

use g3_types::metrics::{NodeName, StaticMetricsTags};
use g3_types::stats::{StatId, TcpIoSnapshot, TcpIoStats};

use crate::serve::{ServerForbiddenSnapshot, ServerForbiddenStats, ServerStats};

pub(crate) struct TcpStreamServerStats {
    name: NodeName,
    id: StatId,

    extra_metrics_tags: Arc<ArcSwapOption<StaticMetricsTags>>,

    online: AtomicIsize,
    conn_total: AtomicU64,

    task_total: AtomicU64,
    task_alive_count: AtomicI32,

    tcp: TcpIoStats,
    pub(crate) forbidden: ServerForbiddenStats,
}

impl TcpStreamServerStats {
    pub(crate) fn new(name: &NodeName) -> Self {
        TcpStreamServerStats {
            name: name.clone(),
            id: StatId::new_unique(),
            extra_metrics_tags: Arc::new(ArcSwapOption::new(None)),
            online: AtomicIsize::new(0),
            conn_total: AtomicU64::new(0),
            task_total: AtomicU64::new(0),
            task_alive_count: AtomicI32::new(0),
            tcp: Default::default(),
            forbidden: Default::default(),
        }
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

    pub(crate) fn add_conn(&self, _addr: SocketAddr) {
        self.conn_total.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub(crate) fn add_read(&self, size: u64) {
        self.tcp.add_in_bytes(size);
    }

    #[inline]
    pub(crate) fn add_write(&self, size: u64) {
        self.tcp.add_out_bytes(size);
    }

    #[must_use]
    pub(crate) fn add_task(self: &Arc<Self>) -> TcpStreamServerAliveTaskGuard {
        self.task_total.fetch_add(1, Ordering::Relaxed);
        self.task_alive_count.fetch_add(1, Ordering::Relaxed);
        TcpStreamServerAliveTaskGuard(self.clone())
    }
}

pub(crate) struct TcpStreamServerAliveTaskGuard(Arc<TcpStreamServerStats>);

impl Drop for TcpStreamServerAliveTaskGuard {
    fn drop(&mut self) {
        self.0.task_alive_count.fetch_sub(1, Ordering::Relaxed);
    }
}

impl ServerStats for TcpStreamServerStats {
    #[inline]
    fn name(&self) -> &NodeName {
        &self.name
    }

    #[inline]
    fn stat_id(&self) -> StatId {
        self.id
    }

    #[inline]
    fn load_extra_tags(&self) -> Option<Arc<StaticMetricsTags>> {
        self.extra_metrics_tags.load_full()
    }

    #[inline]
    fn share_extra_tags(&self) -> &Arc<ArcSwapOption<StaticMetricsTags>> {
        &self.extra_metrics_tags
    }

    fn is_online(&self) -> bool {
        self.online.load(Ordering::Relaxed) > 0
    }

    fn get_conn_total(&self) -> u64 {
        self.conn_total.load(Ordering::Relaxed)
    }

    fn get_task_total(&self) -> u64 {
        self.task_total.load(Ordering::Relaxed)
    }

    fn get_alive_count(&self) -> i32 {
        self.task_alive_count.load(Ordering::Relaxed)
    }

    fn tcp_io_snapshot(&self) -> Option<TcpIoSnapshot> {
        Some(self.tcp.snapshot())
    }

    #[inline]
    fn forbidden_stats(&self) -> ServerForbiddenSnapshot {
        self.forbidden.snapshot()
    }
}
