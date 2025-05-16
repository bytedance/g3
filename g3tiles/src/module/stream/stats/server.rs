/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicIsize, AtomicU64, Ordering};

use arc_swap::ArcSwapOption;

use g3_types::metrics::{MetricTagMap, NodeName};
use g3_types::stats::{StatId, TcpIoSnapshot, TcpIoStats};

use crate::serve::ServerStats;

pub(crate) struct StreamServerStats {
    name: NodeName,
    id: StatId,

    extra_metrics_tags: Arc<ArcSwapOption<MetricTagMap>>,

    online: AtomicIsize,
    conn_total: AtomicU64,

    task_total: AtomicU64,
    task_alive_count: AtomicI32,

    tcp: TcpIoStats,
    // pub(crate) forbidden: ServerForbiddenStats,
}

impl StreamServerStats {
    pub(crate) fn new(name: &NodeName) -> Self {
        StreamServerStats {
            name: name.clone(),
            id: StatId::new_unique(),
            extra_metrics_tags: Arc::new(ArcSwapOption::new(None)),
            online: AtomicIsize::new(0),
            conn_total: AtomicU64::new(0),
            task_total: AtomicU64::new(0),
            task_alive_count: AtomicI32::new(0),
            tcp: Default::default(),
        }
    }

    pub(crate) fn set_online(&self) {
        self.online.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn set_offline(&self) {
        self.online.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn set_extra_tags(&self, tags: Option<Arc<MetricTagMap>>) {
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
    pub(crate) fn add_task(self: &Arc<Self>) -> StreamServerAliveTaskGuard {
        self.task_total.fetch_add(1, Ordering::Relaxed);
        self.task_alive_count.fetch_add(1, Ordering::Relaxed);
        StreamServerAliveTaskGuard(self.clone())
    }
}

pub(crate) struct StreamServerAliveTaskGuard(Arc<StreamServerStats>);

impl Drop for StreamServerAliveTaskGuard {
    fn drop(&mut self) {
        self.0.task_alive_count.fetch_sub(1, Ordering::Relaxed);
    }
}

impl ServerStats for StreamServerStats {
    #[inline]
    fn name(&self) -> &NodeName {
        &self.name
    }

    #[inline]
    fn stat_id(&self) -> StatId {
        self.id
    }

    #[inline]
    fn load_extra_tags(&self) -> Option<Arc<MetricTagMap>> {
        self.extra_metrics_tags.load_full()
    }

    fn is_online(&self) -> bool {
        self.online.load(Ordering::Relaxed) > 0
    }

    fn conn_total(&self) -> u64 {
        self.conn_total.load(Ordering::Relaxed)
    }

    fn task_total(&self) -> u64 {
        self.task_total.load(Ordering::Relaxed)
    }

    fn alive_count(&self) -> i32 {
        self.task_alive_count.load(Ordering::Relaxed)
    }

    fn tcp_io_snapshot(&self) -> Option<TcpIoSnapshot> {
        Some(self.tcp.snapshot())
    }
}
