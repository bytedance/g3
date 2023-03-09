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

use std::convert::TryFrom;
use std::sync::{Arc, Mutex};

use ahash::AHashMap;
use cadence::{Counted, Metric, MetricBuilder, StatsdClient};
use once_cell::sync::Lazy;

use g3_daemon::metric::{
    TAG_KEY_STAT_ID, TAG_KEY_TRANSPORT, TRANSPORT_TYPE_TCP, TRANSPORT_TYPE_UDP,
};
use g3_types::metrics::StaticMetricsTags;
use g3_types::stats::{StatId, TcpIoSnapshot, UdpIoSnapshot};

use super::TAG_KEY_ESCAPER;
use crate::escape::{
    ArcEscaperStats, EscaperForbiddenSnapshot, RouteEscaperSnapshot, RouteEscaperStats,
};

const METRIC_NAME_ESCAPER_TASK_TOTAL: &str = "escaper.task.total";
const METRIC_NAME_ESCAPER_CONN_ATTEMPT: &str = "escaper.connection.attempt";
const METRIC_NAME_ESCAPER_CONN_ESTABLISH: &str = "escaper.connection.establish";
const METRIC_NAME_ESCAPER_IO_IN_BYTES: &str = "escaper.traffic.in.bytes";
const METRIC_NAME_ESCAPER_IO_IN_PACKETS: &str = "escaper.traffic.in.packets";
const METRIC_NAME_ESCAPER_IO_OUT_BYTES: &str = "escaper.traffic.out.bytes";
const METRIC_NAME_ESCAPER_IO_OUT_PACKETS: &str = "escaper.traffic.out.packets";
const METRIC_NAME_ESCAPER_FORBIDDEN_IP_BLOCKED: &str = "escaper.forbidden.ip_blocked";

const METRIC_NAME_ROUTE_REQUEST_PASSED: &str = "route.request.passed";
const METRIC_NAME_ROUTE_REQUEST_FAILED: &str = "route.request.failed";

type EscaperStatsValue = (ArcEscaperStats, EscaperSnapshotStats);
type RouterStatsValue = (Arc<RouteEscaperStats>, RouteEscaperSnapshot);

static ESCAPER_STATS_MAP: Lazy<Mutex<AHashMap<StatId, EscaperStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));
static ROUTE_STATS_MAP: Lazy<Mutex<AHashMap<StatId, RouterStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));

trait EscaperMetricExt<'m> {
    fn add_escaper_tags(self, escaper: &'m str, stat_id: &'m str) -> Self;
    fn add_escaper_extra_tags(self, tags: &'m Option<Arc<StaticMetricsTags>>) -> Self;
}

impl<'m, 'c, T> EscaperMetricExt<'m> for MetricBuilder<'m, 'c, T>
where
    T: Metric + From<String>,
{
    fn add_escaper_tags(self, escaper: &'m str, stat_id: &'m str) -> Self {
        self.with_tag(TAG_KEY_ESCAPER, escaper)
            .with_tag(TAG_KEY_STAT_ID, stat_id)
    }

    fn add_escaper_extra_tags(mut self, tags: &'m Option<Arc<StaticMetricsTags>>) -> Self {
        if let Some(tags) = tags {
            for (k, v) in tags.iter() {
                self = self.with_tag(k.as_str(), v.as_str());
            }
        }
        self
    }
}

#[derive(Default)]
struct EscaperSnapshotStats {
    task_total: u64,
    conn_attempt: u64,
    conn_establish: u64,
    tcp: TcpIoSnapshot,
    udp: UdpIoSnapshot,
    forbidden: EscaperForbiddenSnapshot,
}

pub(in crate::stat) fn sync_stats() {
    let mut escaper_stats_map = ESCAPER_STATS_MAP.lock().unwrap();
    crate::escape::foreach_escaper(|_, escaper| {
        if let Some(stats) = escaper.get_escape_stats() {
            let stat_id = stats.stat_id();
            escaper_stats_map
                .entry(stat_id)
                .or_insert_with(|| (stats, EscaperSnapshotStats::default()));
        }
    });
    drop(escaper_stats_map);

    let mut route_stats_map = ROUTE_STATS_MAP.lock().unwrap();
    crate::escape::foreach_escaper(|_, escaper| {
        if let Some(stats) = escaper.ref_route_stats() {
            let stats = Arc::clone(stats);
            let stat_id = stats.stat_id();
            route_stats_map
                .entry(stat_id)
                .or_insert_with(|| (stats, RouteEscaperSnapshot::default()));
        }
    });
    drop(route_stats_map);
}

pub(in crate::stat) fn emit_stats(client: &StatsdClient) {
    let mut escaper_stats_map = ESCAPER_STATS_MAP.lock().unwrap();
    escaper_stats_map.retain(|_, (stats, snap)| {
        emit_escaper_stats(client, stats, snap);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
    drop(escaper_stats_map);

    let mut route_stats_map = ROUTE_STATS_MAP.lock().unwrap();
    route_stats_map.retain(|_, (stats, snap)| {
        emit_route_stats(client, stats, snap);
        Arc::strong_count(stats) > 1
    });
    drop(route_stats_map);
}

fn emit_escaper_stats(
    client: &StatsdClient,
    stats: &ArcEscaperStats,
    snap: &mut EscaperSnapshotStats,
) {
    let escaper = stats.name();
    let mut buffer = itoa::Buffer::new();
    let stat_id = buffer.format(stats.stat_id().as_u64());

    let guard = stats.extra_tags().load();
    let escaper_extra_tags = guard.as_ref().map(Arc::clone);
    drop(guard);

    let new_value = stats.get_task_total();
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.task_total)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_ESCAPER_TASK_TOTAL, diff_value)
        .add_escaper_tags(escaper, stat_id)
        .add_escaper_extra_tags(&escaper_extra_tags)
        .send();
    snap.task_total = new_value;

    let new_value = stats.get_conn_attempted();
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.conn_attempt)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_ESCAPER_CONN_ATTEMPT, diff_value)
        .add_escaper_tags(escaper, stat_id)
        .add_escaper_extra_tags(&escaper_extra_tags)
        .send();
    snap.conn_attempt = new_value;

    let new_value = stats.get_conn_established();
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.conn_establish)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_ESCAPER_CONN_ESTABLISH, diff_value)
        .add_escaper_tags(escaper, stat_id)
        .add_escaper_extra_tags(&escaper_extra_tags)
        .send();
    snap.conn_establish = new_value;

    if let Some(forbidden_stats) = stats.forbidden_snapshot() {
        emit_forbidden_stats(
            client,
            forbidden_stats,
            &mut snap.forbidden,
            escaper,
            &escaper_extra_tags,
            stat_id,
        );
    }

    if let Some(tcp_io_stats) = stats.tcp_io_snapshot() {
        emit_tcp_io_to_statsd(
            client,
            tcp_io_stats,
            &mut snap.tcp,
            escaper,
            &escaper_extra_tags,
            stat_id,
        );
    }

    if let Some(udp_io_stats) = stats.udp_io_snapshot() {
        emit_udp_io_to_statsd(
            client,
            udp_io_stats,
            &mut snap.udp,
            escaper,
            &escaper_extra_tags,
            stat_id,
        );
    }
}

fn emit_forbidden_stats(
    client: &StatsdClient,
    stats: EscaperForbiddenSnapshot,
    snap: &mut EscaperForbiddenSnapshot,
    escaper: &str,
    escaper_extra_tags: &Option<Arc<StaticMetricsTags>>,
    stat_id: &str,
) {
    let new_value = stats.ip_blocked;
    if new_value != 0 || snap.ip_blocked != 0 {
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.ip_blocked)).unwrap_or(i64::MAX);
        client
            .count_with_tags(METRIC_NAME_ESCAPER_FORBIDDEN_IP_BLOCKED, diff_value)
            .add_escaper_tags(escaper, stat_id)
            .add_escaper_extra_tags(escaper_extra_tags)
            .send();
        snap.ip_blocked = new_value;
    }
}

fn emit_tcp_io_to_statsd(
    client: &StatsdClient,
    stats: TcpIoSnapshot,
    snap: &mut TcpIoSnapshot,
    escaper: &str,
    escaper_extra_tags: &Option<Arc<StaticMetricsTags>>,
    stat_id: &str,
) {
    let new_value = stats.out_bytes;
    if new_value == 0 && snap.out_bytes == 0 {
        return;
    }
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.out_bytes)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_ESCAPER_IO_OUT_BYTES, diff_value)
        .add_escaper_tags(escaper, stat_id)
        .add_escaper_extra_tags(escaper_extra_tags)
        .with_tag(TAG_KEY_TRANSPORT, TRANSPORT_TYPE_TCP)
        .send();
    snap.out_bytes = new_value;

    let new_value = stats.in_bytes;
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.in_bytes)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_ESCAPER_IO_IN_BYTES, diff_value)
        .add_escaper_tags(escaper, stat_id)
        .add_escaper_extra_tags(escaper_extra_tags)
        .with_tag(TAG_KEY_TRANSPORT, TRANSPORT_TYPE_TCP)
        .send();
    snap.in_bytes = new_value;
}

fn emit_udp_io_to_statsd(
    client: &StatsdClient,
    stats: UdpIoSnapshot,
    snap: &mut UdpIoSnapshot,
    escaper: &str,
    escaper_extra_tags: &Option<Arc<StaticMetricsTags>>,
    stat_id: &str,
) {
    let new_value = stats.out_packets;
    if new_value == 0 && snap.out_packets == 0 {
        return;
    }
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.out_packets)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_ESCAPER_IO_OUT_PACKETS, diff_value)
        .add_escaper_tags(escaper, stat_id)
        .add_escaper_extra_tags(escaper_extra_tags)
        .with_tag(TAG_KEY_TRANSPORT, TRANSPORT_TYPE_UDP)
        .send();
    snap.out_packets = new_value;

    let new_value = stats.out_bytes;
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.out_bytes)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_ESCAPER_IO_OUT_BYTES, diff_value)
        .add_escaper_tags(escaper, stat_id)
        .add_escaper_extra_tags(escaper_extra_tags)
        .with_tag(TAG_KEY_TRANSPORT, TRANSPORT_TYPE_UDP)
        .send();
    snap.out_bytes = new_value;

    let new_value = stats.in_packets;
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.in_packets)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_ESCAPER_IO_IN_PACKETS, diff_value)
        .add_escaper_tags(escaper, stat_id)
        .add_escaper_extra_tags(escaper_extra_tags)
        .with_tag(TAG_KEY_TRANSPORT, TRANSPORT_TYPE_UDP)
        .send();
    snap.in_packets = new_value;

    let new_value = stats.in_bytes;
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.in_bytes)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_ESCAPER_IO_IN_BYTES, diff_value)
        .add_escaper_tags(escaper, stat_id)
        .add_escaper_extra_tags(escaper_extra_tags)
        .with_tag(TAG_KEY_TRANSPORT, TRANSPORT_TYPE_UDP)
        .send();
    snap.in_bytes = new_value;
}

fn emit_route_stats(
    client: &StatsdClient,
    stats: &Arc<RouteEscaperStats>,
    snap: &mut RouteEscaperSnapshot,
) {
    let escaper = stats.name();
    let mut buffer = itoa::Buffer::new();
    let stat_id = buffer.format(stats.stat_id().as_u64());

    let stats = stats.snapshot();

    let new_value = stats.request_passed;
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.request_passed)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_ROUTE_REQUEST_PASSED, diff_value)
        .add_escaper_tags(escaper, stat_id)
        .send();
    snap.request_passed = new_value;

    let new_value = stats.request_failed;
    if new_value != 0 || snap.request_failed != 0 {
        let diff_value =
            i64::try_from(new_value.wrapping_sub(snap.request_failed)).unwrap_or(i64::MAX);
        client
            .count_with_tags(METRIC_NAME_ROUTE_REQUEST_FAILED, diff_value)
            .add_escaper_tags(escaper, stat_id)
            .send();
        snap.request_failed = new_value;
    }
}
