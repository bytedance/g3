/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};

use arc_swap::ArcSwapOption;

use g3_histogram::{HistogramMetricsConfig, HistogramRecorder, HistogramStats};
use g3_types::metrics::{NodeName, StaticMetricsTags};
use g3_types::stats::StatId;

pub(crate) struct KeylessBackendStats {
    name: NodeName,
    id: StatId,
    extra_metrics_tags: Arc<ArcSwapOption<StaticMetricsTags>>,

    conn_attempt: AtomicU64,
    conn_established: AtomicU64,

    alive_channel: AtomicI32,

    request_recv: AtomicU64,
    request_send: AtomicU64,
    request_drop: AtomicU64,
    request_timeout: AtomicU64,
    response_recv: AtomicU64,
    response_send: AtomicU64,
    response_drop: AtomicU64,
}

impl KeylessBackendStats {
    pub(crate) fn new(name: &NodeName) -> Self {
        KeylessBackendStats {
            name: name.clone(),
            id: StatId::new(),
            extra_metrics_tags: Arc::new(ArcSwapOption::new(None)),
            conn_attempt: AtomicU64::new(0),
            conn_established: AtomicU64::new(0),
            alive_channel: AtomicI32::new(0),
            request_recv: AtomicU64::new(0),
            request_send: AtomicU64::new(0),
            request_drop: AtomicU64::new(0),
            request_timeout: AtomicU64::new(0),
            response_recv: AtomicU64::new(0),
            response_send: AtomicU64::new(0),
            response_drop: AtomicU64::new(0),
        }
    }

    pub(crate) fn set_extra_tags(&self, tags: Option<Arc<StaticMetricsTags>>) {
        self.extra_metrics_tags.store(tags);
    }

    pub(crate) fn load_extra_tags(&self) -> Option<Arc<StaticMetricsTags>> {
        self.extra_metrics_tags.load_full()
    }

    #[inline]
    pub(crate) fn name(&self) -> &NodeName {
        &self.name
    }

    #[inline]
    pub(crate) fn stat_id(&self) -> StatId {
        self.id
    }

    pub(crate) fn add_conn_attempt(&self) {
        self.conn_attempt.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn conn_attempt(&self) -> u64 {
        self.conn_attempt.load(Ordering::Relaxed)
    }

    pub(crate) fn add_conn_established(&self) {
        self.conn_established.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn conn_established(&self) -> u64 {
        self.conn_established.load(Ordering::Relaxed)
    }

    pub(crate) fn inc_alive_channel(self: &Arc<Self>) -> KeylessBackendAliveChannelGuard {
        self.alive_channel.fetch_add(1, Ordering::Relaxed);
        KeylessBackendAliveChannelGuard(self.clone())
    }

    pub(crate) fn alive_channel(&self) -> i32 {
        self.alive_channel.load(Ordering::Relaxed)
    }

    pub(crate) fn add_request_recv(&self) {
        self.request_recv.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn request_recv(&self) -> u64 {
        self.request_recv.load(Ordering::Relaxed)
    }

    pub(crate) fn add_request_send(&self) {
        self.request_send.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn request_send(&self) -> u64 {
        self.request_send.load(Ordering::Relaxed)
    }

    pub(crate) fn add_request_drop(&self) {
        self.request_drop.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn request_drop(&self) -> u64 {
        self.request_drop.load(Ordering::Relaxed)
    }

    pub(crate) fn add_request_timeout(&self) {
        self.request_timeout.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn request_timeout(&self) -> u64 {
        self.request_timeout.load(Ordering::Relaxed)
    }

    pub(crate) fn add_response_recv(&self) {
        self.response_recv.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn response_recv(&self) -> u64 {
        self.response_recv.load(Ordering::Relaxed)
    }

    pub(crate) fn add_response_send(&self) {
        self.response_send.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn response_send(&self) -> u64 {
        self.response_send.load(Ordering::Relaxed)
    }

    pub(crate) fn add_response_drop(&self) {
        self.response_drop.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn response_drop(&self) -> u64 {
        self.response_drop.load(Ordering::Relaxed)
    }
}

pub(crate) struct KeylessBackendAliveChannelGuard(Arc<KeylessBackendStats>);

impl Drop for KeylessBackendAliveChannelGuard {
    fn drop(&mut self) {
        self.0.alive_channel.fetch_sub(1, Ordering::Relaxed);
    }
}

pub(crate) struct KeylessUpstreamDurationStats {
    name: NodeName,
    id: StatId,
    extra_metrics_tags: Arc<ArcSwapOption<StaticMetricsTags>>,

    pub(crate) connect: Arc<HistogramStats>,
    pub(crate) wait: Arc<HistogramStats>,
    pub(crate) response: Arc<HistogramStats>,
}

impl KeylessUpstreamDurationStats {
    pub(crate) fn set_extra_tags(&self, tags: Option<Arc<StaticMetricsTags>>) {
        self.extra_metrics_tags.store(tags);
    }

    pub(crate) fn load_extra_tags(&self) -> Option<Arc<StaticMetricsTags>> {
        self.extra_metrics_tags.load_full()
    }

    #[inline]
    pub(crate) fn name(&self) -> &NodeName {
        &self.name
    }

    #[inline]
    pub(crate) fn stat_id(&self) -> StatId {
        self.id
    }
}

pub(crate) struct KeylessUpstreamDurationRecorder {
    pub(crate) connect: HistogramRecorder<u64>,
    pub(crate) wait: HistogramRecorder<u64>,
    pub(crate) response: HistogramRecorder<u64>,
}

impl KeylessUpstreamDurationRecorder {
    pub(crate) fn new(
        name: &NodeName,
        config: &HistogramMetricsConfig,
    ) -> (
        KeylessUpstreamDurationRecorder,
        KeylessUpstreamDurationStats,
    ) {
        let rt_handle = g3_daemon::runtime::main_handle();
        let (connect_r, connect_s) = config.build_spawned(rt_handle.cloned());
        let (wait_r, wait_s) = config.build_spawned(rt_handle.cloned());
        let (response_r, response_s) = config.build_spawned(rt_handle.cloned());
        let r = KeylessUpstreamDurationRecorder {
            connect: connect_r,
            wait: wait_r,
            response: response_r,
        };
        let s = KeylessUpstreamDurationStats {
            name: name.clone(),
            id: StatId::new(),
            extra_metrics_tags: Arc::new(ArcSwapOption::new(None)),
            connect: connect_s,
            wait: wait_s,
            response: response_s,
        };
        (r, s)
    }
}
