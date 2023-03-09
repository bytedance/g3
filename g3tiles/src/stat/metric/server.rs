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

use std::convert::{From, TryFrom};
use std::sync::{Arc, Mutex};

use ahash::AHashMap;
use cadence::{Counted, Gauged, Metric, MetricBuilder, StatsdClient};
use once_cell::sync::Lazy;

use g3_daemon::listen::{ListenSnapshot, ListenStats};
use g3_daemon::metric::{
    TAG_KEY_STAT_ID, TAG_KEY_TRANSPORT, TRANSPORT_TYPE_TCP, TRANSPORT_TYPE_UDP,
};
use g3_types::metrics::StaticMetricsTags;
use g3_types::stats::{StatId, TcpIoSnapshot, UdpIoSnapshot};

use crate::serve::ArcServerStats;

use super::TAG_KEY_SERVER;

const TAG_KEY_ONLINE: &str = "online";

const METRIC_NAME_SERVER_CONN_TOTAL: &str = "server.connection.total";
const METRIC_NAME_SERVER_TASK_TOTAL: &str = "server.task.total";
const METRIC_NAME_SERVER_TASK_ALIVE: &str = "server.task.alive";
const METRIC_NAME_SERVER_IO_IN_BYTES: &str = "server.traffic.in.bytes";
const METRIC_NAME_SERVER_IO_IN_PACKETS: &str = "server.traffic.in.packets";
const METRIC_NAME_SERVER_IO_OUT_BYTES: &str = "server.traffic.out.bytes";
const METRIC_NAME_SERVER_IO_OUT_PACKETS: &str = "server.traffic.out.packets";

const METRIC_NAME_LISTEN_INSTANCE_COUNT: &str = "listen.instance.count";
const METRIC_NAME_LISTEN_ACCEPTED: &str = "listen.accepted";
const METRIC_NAME_LISTEN_DROPPED: &str = "listen.dropped";
const METRIC_NAME_LISTEN_TIMEOUT: &str = "listen.timeout";
const METRIC_NAME_LISTEN_FAILED: &str = "listen.failed";

type ServerStatsValue = (ArcServerStats, ServerSnapshot);
type ListenStatsValue = (Arc<ListenStats>, ListenSnapshot);

static SERVER_STATS_MAP: Lazy<Mutex<AHashMap<StatId, ServerStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));
static LISTEN_STATS_MAP: Lazy<Mutex<AHashMap<StatId, ListenStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));

trait ServerMetricExt<'m> {
    fn add_server_tags(self, server: &'m str, online_value: &'m str, stat_id: &'m str) -> Self;
    fn add_server_extra_tags(self, tags: &'m Option<Arc<StaticMetricsTags>>) -> Self;
}

impl<'m, 'c, T> ServerMetricExt<'m> for MetricBuilder<'m, 'c, T>
where
    T: Metric + From<String>,
{
    fn add_server_tags(self, server: &'m str, online_value: &'m str, stat_id: &'m str) -> Self {
        self.with_tag(TAG_KEY_SERVER, server)
            .with_tag(TAG_KEY_ONLINE, online_value)
            .with_tag(TAG_KEY_STAT_ID, stat_id)
    }

    fn add_server_extra_tags(mut self, tags: &'m Option<Arc<StaticMetricsTags>>) -> Self {
        if let Some(tags) = tags {
            for (k, v) in tags.iter() {
                self = self.with_tag(k.as_str(), v.as_str());
            }
        }
        self
    }
}

#[derive(Default)]
struct ServerSnapshot {
    conn_total: u64,
    task_total: u64,
    tcp: TcpIoSnapshot,
    udp: UdpIoSnapshot,
}

pub(in crate::stat) fn sync_stats() {
    let mut server_stats_map = SERVER_STATS_MAP.lock().unwrap();
    crate::serve::foreach_server(|_, server| {
        if let Some(stats) = server.get_server_stats() {
            let stat_id = stats.stat_id();
            server_stats_map
                .entry(stat_id)
                .or_insert_with(|| (stats, ServerSnapshot::default()));
        }
    });
    drop(server_stats_map);

    let mut listen_stats_map = LISTEN_STATS_MAP.lock().unwrap();
    crate::serve::foreach_server(|_, server| {
        let stats = server.get_listen_stats();
        let stat_id = stats.stat_id();
        listen_stats_map
            .entry(stat_id)
            .or_insert_with(|| (stats, ListenSnapshot::default()));
    });
    drop(listen_stats_map);
}

pub(in crate::stat) fn emit_stats(client: &StatsdClient) {
    let mut server_stats_map = SERVER_STATS_MAP.lock().unwrap();
    server_stats_map.retain(|_, (stats, snap)| {
        emit_server_stats(client, stats, snap);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
    drop(server_stats_map);

    let mut listen_stats_map = LISTEN_STATS_MAP.lock().unwrap();
    listen_stats_map.retain(|_, (stats, snap)| {
        emit_listen_stats(client, stats, snap);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
}

fn emit_server_stats(client: &StatsdClient, stats: &ArcServerStats, snap: &mut ServerSnapshot) {
    let online_value = if stats.is_online() { "y" } else { "n" };
    let server = stats.name();
    let mut buffer = itoa::Buffer::new();
    let stat_id = buffer.format(stats.stat_id().as_u64());

    let guard = stats.extra_tags().load();
    let server_extra_tags = guard.as_ref().map(Arc::clone);
    drop(guard);

    let new_value = stats.get_conn_total();
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.conn_total)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_SERVER_CONN_TOTAL, diff_value)
        .add_server_tags(server, online_value, stat_id)
        .add_server_extra_tags(&server_extra_tags)
        .send();
    snap.conn_total = new_value;

    let new_value = stats.get_task_total();
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.task_total)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_SERVER_TASK_TOTAL, diff_value)
        .add_server_tags(server, online_value, stat_id)
        .add_server_extra_tags(&server_extra_tags)
        .send();
    snap.task_total = new_value;

    client
        .gauge_with_tags(
            METRIC_NAME_SERVER_TASK_ALIVE,
            stats.get_alive_count() as f64,
        )
        .add_server_tags(server, online_value, stat_id)
        .add_server_extra_tags(&server_extra_tags)
        .send();

    if let Some(tcp_io_stats) = stats.tcp_io_snapshot() {
        emit_tcp_io_to_statsd(
            client,
            tcp_io_stats,
            &mut snap.tcp,
            online_value,
            server,
            &server_extra_tags,
            stat_id,
        );
    }

    if let Some(udp_io_stats) = stats.udp_io_snapshot() {
        emit_udp_io_to_statsd(
            client,
            udp_io_stats,
            &mut snap.udp,
            online_value,
            server,
            &server_extra_tags,
            stat_id,
        );
    }
}

fn emit_tcp_io_to_statsd(
    client: &StatsdClient,
    stats: TcpIoSnapshot,
    snap: &mut TcpIoSnapshot,
    online_value: &str,
    server: &str,
    server_extra_tags: &Option<Arc<StaticMetricsTags>>,
    stat_id: &str,
) {
    let new_value = stats.in_bytes;
    if new_value == 0 && snap.in_bytes == 0 {
        return;
    }
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.in_bytes)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_SERVER_IO_IN_BYTES, diff_value)
        .add_server_tags(server, online_value, stat_id)
        .add_server_extra_tags(server_extra_tags)
        .with_tag(TAG_KEY_TRANSPORT, TRANSPORT_TYPE_TCP)
        .send();
    snap.in_bytes = new_value;

    let new_value = stats.out_bytes;
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.out_bytes)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_SERVER_IO_OUT_BYTES, diff_value)
        .add_server_tags(server, online_value, stat_id)
        .add_server_extra_tags(server_extra_tags)
        .with_tag(TAG_KEY_TRANSPORT, TRANSPORT_TYPE_TCP)
        .send();
    snap.out_bytes = new_value;
}

fn emit_udp_io_to_statsd(
    client: &StatsdClient,
    stats: UdpIoSnapshot,
    snap: &mut UdpIoSnapshot,
    online_value: &str,
    server: &str,
    server_extra_tags: &Option<Arc<StaticMetricsTags>>,
    stat_id: &str,
) {
    let new_value = stats.in_packets;
    if new_value == 0 && snap.in_packets == 0 {
        return;
    }
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.in_packets)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_SERVER_IO_IN_PACKETS, diff_value)
        .add_server_tags(server, online_value, stat_id)
        .add_server_extra_tags(server_extra_tags)
        .with_tag(TAG_KEY_TRANSPORT, TRANSPORT_TYPE_UDP)
        .send();
    snap.in_packets = new_value;

    let new_value = stats.in_bytes;
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.in_bytes)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_SERVER_IO_IN_BYTES, diff_value)
        .add_server_tags(server, online_value, stat_id)
        .add_server_extra_tags(server_extra_tags)
        .with_tag(TAG_KEY_TRANSPORT, TRANSPORT_TYPE_UDP)
        .send();
    snap.in_bytes = new_value;

    let new_value = stats.out_packets;
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.out_packets)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_SERVER_IO_OUT_PACKETS, diff_value)
        .add_server_tags(server, online_value, stat_id)
        .add_server_extra_tags(server_extra_tags)
        .with_tag(TAG_KEY_TRANSPORT, TRANSPORT_TYPE_UDP)
        .send();
    snap.out_packets = new_value;

    let new_value = stats.out_bytes;
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.out_bytes)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_SERVER_IO_OUT_BYTES, diff_value)
        .add_server_tags(server, online_value, stat_id)
        .add_server_extra_tags(server_extra_tags)
        .with_tag(TAG_KEY_TRANSPORT, TRANSPORT_TYPE_UDP)
        .send();
    snap.out_bytes = new_value;
}

fn emit_listen_stats(client: &StatsdClient, stats: &Arc<ListenStats>, snap: &mut ListenSnapshot) {
    let online_value = if stats.is_running() { "y" } else { "n" };
    let server = stats.name();
    let mut buffer = itoa::Buffer::new();
    let stat_id = buffer.format(stats.stat_id().as_u64());

    client
        .gauge_with_tags(
            METRIC_NAME_LISTEN_INSTANCE_COUNT,
            stats.get_running_runtime_count() as f64,
        )
        .add_server_tags(server, online_value, stat_id)
        .send();

    let new_value = stats.get_accepted();
    if new_value != 0 || snap.accepted != 0 {
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.accepted)).unwrap_or(i64::MAX);
        client
            .count_with_tags(METRIC_NAME_LISTEN_ACCEPTED, diff_value)
            .add_server_tags(server, online_value, stat_id)
            .send();
        snap.accepted = new_value;
    }

    let new_value = stats.get_dropped();
    if new_value != 0 || snap.dropped != 0 {
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.dropped)).unwrap_or(i64::MAX);
        client
            .count_with_tags(METRIC_NAME_LISTEN_DROPPED, diff_value)
            .add_server_tags(server, online_value, stat_id)
            .send();
        snap.dropped = new_value;
    }

    let new_value = stats.get_timeout();
    if new_value != 0 || snap.timeout != 0 {
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.timeout)).unwrap_or(i64::MAX);
        client
            .count_with_tags(METRIC_NAME_LISTEN_TIMEOUT, diff_value)
            .add_server_tags(server, online_value, stat_id)
            .send();
        snap.timeout = new_value;
    }

    let new_value = stats.get_failed();
    if new_value != 0 || snap.failed != 0 {
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.failed)).unwrap_or(i64::MAX);
        client
            .count_with_tags(METRIC_NAME_LISTEN_FAILED, diff_value)
            .add_server_tags(server, online_value, stat_id)
            .send();
        snap.failed = new_value;
    }
}
