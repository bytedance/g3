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
use std::sync::atomic::{AtomicIsize, AtomicU64, Ordering};
use std::sync::Arc;

use arc_swap::ArcSwapOption;

use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_types::stats::{StatId, TcpIoSnapshot, TcpIoStats, UdpIoSnapshot, UdpIoStats};

use crate::serve::{
    ServerForbiddenSnapshot, ServerForbiddenStats, ServerPerTaskStats, ServerStats,
};

pub(crate) struct SocksProxyServerStats {
    name: MetricsName,
    id: StatId,

    extra_metrics_tags: Arc<ArcSwapOption<StaticMetricsTags>>,

    online: AtomicIsize,
    conn_total: AtomicU64,

    pub(crate) forbidden: ServerForbiddenStats,

    pub(crate) task_tcp_connect: ServerPerTaskStats,
    pub(crate) task_udp_associate: ServerPerTaskStats,
    pub(crate) task_udp_connect: ServerPerTaskStats,

    pub(crate) io_tcp: TcpIoStats,
    pub(crate) io_udp: UdpIoStats,
}

impl SocksProxyServerStats {
    pub(crate) fn new(name: &MetricsName) -> Self {
        SocksProxyServerStats {
            name: name.clone(),
            id: StatId::new(),
            extra_metrics_tags: Arc::new(ArcSwapOption::new(None)),
            online: AtomicIsize::new(0),
            conn_total: AtomicU64::new(0),
            forbidden: Default::default(),
            task_tcp_connect: Default::default(),
            task_udp_associate: Default::default(),
            task_udp_connect: Default::default(),
            io_tcp: TcpIoStats::default(),
            io_udp: UdpIoStats::default(),
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
}

impl ServerStats for SocksProxyServerStats {
    #[inline]
    fn name(&self) -> &MetricsName {
        &self.name
    }

    #[inline]
    fn stat_id(&self) -> StatId {
        self.id
    }

    #[inline]
    fn extra_tags(&self) -> &Arc<ArcSwapOption<StaticMetricsTags>> {
        &self.extra_metrics_tags
    }

    fn is_online(&self) -> bool {
        self.online.load(Ordering::Relaxed) > 0
    }

    fn get_conn_total(&self) -> u64 {
        self.conn_total.load(Ordering::Relaxed)
    }

    fn get_task_total(&self) -> u64 {
        self.task_tcp_connect.get_task_total()
            + self.task_udp_connect.get_task_total()
            + self.task_udp_associate.get_task_total()
    }

    fn get_alive_count(&self) -> i32 {
        self.task_tcp_connect.get_alive_count()
            + self.task_udp_connect.get_alive_count()
            + self.task_udp_associate.get_alive_count()
    }

    #[inline]
    fn tcp_io_snapshot(&self) -> Option<TcpIoSnapshot> {
        Some(self.io_tcp.snapshot())
    }

    #[inline]
    fn udp_io_snapshot(&self) -> Option<UdpIoSnapshot> {
        Some(self.io_udp.snapshot())
    }

    #[inline]
    fn forbidden_stats(&self) -> ServerForbiddenSnapshot {
        self.forbidden.snapshot()
    }
}
