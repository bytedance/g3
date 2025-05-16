/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, AtomicU64, Ordering};

use arc_swap::ArcSwapOption;

use g3_types::metrics::{MetricTagMap, NodeName};
use g3_types::stats::{StatId, TcpIoSnapshot, TcpIoStats};

use crate::serve::{
    ServerForbiddenSnapshot, ServerForbiddenStats, ServerPerTaskStats, ServerStats,
};
use crate::stat::types::UntrustedTaskStatsSnapshot;

pub(crate) struct HttpRProxyServerStats {
    name: NodeName,
    id: StatId,

    extra_metrics_tags: Arc<ArcSwapOption<MetricTagMap>>,

    online: AtomicIsize,
    conn_total: AtomicU64,

    pub forbidden: ServerForbiddenStats,

    pub task_http_untrusted: ServerPerTaskStats,
    pub task_http_forward: ServerPerTaskStats,

    pub io_http: TcpIoStats,
    pub io_untrusted: TcpIoStats,
}

impl HttpRProxyServerStats {
    pub(super) fn new(name: &NodeName) -> Self {
        HttpRProxyServerStats {
            name: name.clone(),
            id: StatId::new_unique(),
            extra_metrics_tags: Arc::new(ArcSwapOption::new(None)),
            online: AtomicIsize::new(0),
            conn_total: AtomicU64::new(0),
            forbidden: Default::default(),
            task_http_untrusted: Default::default(),
            task_http_forward: Default::default(),
            io_http: Default::default(),
            io_untrusted: Default::default(),
        }
    }

    pub(super) fn set_online(&self) {
        self.online.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn set_offline(&self) {
        self.online.fetch_sub(1, Ordering::Relaxed);
    }

    pub(super) fn set_extra_tags(&self, tags: Option<Arc<MetricTagMap>>) {
        self.extra_metrics_tags.store(tags);
    }

    pub(super) fn add_conn(&self, _addr: SocketAddr) {
        self.conn_total.fetch_add(1, Ordering::Relaxed);
    }
}

impl ServerStats for HttpRProxyServerStats {
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

    #[inline]
    fn share_extra_tags(&self) -> &Arc<ArcSwapOption<MetricTagMap>> {
        &self.extra_metrics_tags
    }

    fn is_online(&self) -> bool {
        self.online.load(Ordering::Relaxed) > 0
    }

    fn get_conn_total(&self) -> u64 {
        self.conn_total.load(Ordering::Relaxed)
    }

    fn get_task_total(&self) -> u64 {
        // untrusted stats is not counted in
        self.task_http_forward.get_task_total()
    }

    fn get_alive_count(&self) -> i32 {
        // untrusted stats is not counted in
        self.task_http_forward.get_alive_count()
    }

    fn tcp_io_snapshot(&self) -> Option<TcpIoSnapshot> {
        // the untrusted read stats is collected as buffer stats,
        // which has been contained in io_http
        Some(self.io_http.snapshot())
    }

    #[inline]
    fn forbidden_stats(&self) -> ServerForbiddenSnapshot {
        self.forbidden.snapshot()
    }

    fn untrusted_snapshot(&self) -> Option<UntrustedTaskStatsSnapshot> {
        Some(UntrustedTaskStatsSnapshot {
            task_total: self.task_http_untrusted.get_task_total(),
            task_alive: self.task_http_untrusted.get_alive_count(),
            in_bytes: self.io_untrusted.get_in_bytes(),
        })
    }
}
