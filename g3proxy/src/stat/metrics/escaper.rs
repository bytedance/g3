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

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use foldhash::fast::FixedState;

use g3_daemon::metrics::{
    TAG_KEY_STAT_ID, TAG_KEY_TRANSPORT, TRANSPORT_TYPE_TCP, TRANSPORT_TYPE_UDP,
};
use g3_statsd_client::{StatsdClient, StatsdTagGroup};
use g3_types::metrics::NodeName;
use g3_types::stats::{StatId, TcpIoSnapshot, UdpIoSnapshot};

use super::TAG_KEY_ESCAPER;
use crate::escape::{
    ArcEscaperStats, EscaperForbiddenSnapshot, EscaperTcpConnectSnapshot, EscaperTlsSnapshot,
    RouteEscaperSnapshot, RouteEscaperStats,
};

const METRIC_NAME_ESCAPER_TASK_TOTAL: &str = "escaper.task.total";
const METRIC_NAME_ESCAPER_CONN_ATTEMPT: &str = "escaper.connection.attempt";
const METRIC_NAME_ESCAPER_CONN_ESTABLISH: &str = "escaper.connection.establish";
const METRIC_NAME_ESCAPER_TCP_CONNECT_ATTEMPT: &str = "escaper.tcp.connect.attempt";
const METRIC_NAME_ESCAPER_TCP_CONNECT_ESTABLISH: &str = "escaper.tcp.connect.establish";
const METRIC_NAME_ESCAPER_TCP_CONNECT_SUCCESS: &str = "escaper.tcp.connect.success";
const METRIC_NAME_ESCAPER_TCP_CONNECT_ERROR: &str = "escaper.tcp.connect.error";
const METRIC_NAME_ESCAPER_TCP_CONNECT_TIMEOUT: &str = "escaper.tcp.connect.timeout";
const METRIC_NAME_ESCAPER_TLS_HANDSHAKE_SUCCESS: &str = "escaper.tls.handshake.success";
const METRIC_NAME_ESCAPER_TLS_HANDSHAKE_ERROR: &str = "escaper.tls.handshake.error";
const METRIC_NAME_ESCAPER_TLS_HANDSHAKE_TIMEOUT: &str = "escaper.tls.handshake.timeout";
const METRIC_NAME_ESCAPER_TLS_PEER_ORDERLY_CLOSURE: &str = "escaper.tls.peer.closure.orderly";
const METRIC_NAME_ESCAPER_TLS_PEER_ABORTIVE_CLOSURE: &str = "escaper.tls.peer.closure.abortive";
const METRIC_NAME_ESCAPER_IO_IN_BYTES: &str = "escaper.traffic.in.bytes";
const METRIC_NAME_ESCAPER_IO_IN_PACKETS: &str = "escaper.traffic.in.packets";
const METRIC_NAME_ESCAPER_IO_OUT_BYTES: &str = "escaper.traffic.out.bytes";
const METRIC_NAME_ESCAPER_IO_OUT_PACKETS: &str = "escaper.traffic.out.packets";
const METRIC_NAME_ESCAPER_FORBIDDEN_IP_BLOCKED: &str = "escaper.forbidden.ip_blocked";

const METRIC_NAME_ROUTE_REQUEST_PASSED: &str = "route.request.passed";
const METRIC_NAME_ROUTE_REQUEST_FAILED: &str = "route.request.failed";

type EscaperStatsValue = (ArcEscaperStats, EscaperSnapshot);
type RouterStatsValue = (Arc<RouteEscaperStats>, RouteEscaperSnapshot);

static ESCAPER_STATS_MAP: Mutex<HashMap<StatId, EscaperStatsValue, FixedState>> =
    Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));
static ROUTE_STATS_MAP: Mutex<HashMap<StatId, RouterStatsValue, FixedState>> =
    Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));

trait EscaperMetricExt {
    fn add_escaper_tags(&mut self, escaper: &NodeName, stat_id: StatId);
}

impl EscaperMetricExt for StatsdTagGroup {
    fn add_escaper_tags(&mut self, escaper: &NodeName, stat_id: StatId) {
        let mut buffer = itoa::Buffer::new();
        let stat_id = buffer.format(stat_id.as_u64());
        self.add_tag(TAG_KEY_ESCAPER, escaper);
        self.add_tag(TAG_KEY_STAT_ID, stat_id);
    }
}

#[derive(Default)]
struct EscaperSnapshot {
    task_total: u64,
    conn_attempt: u64,
    conn_establish: u64,
    tcp_connect: EscaperTcpConnectSnapshot,
    tls: EscaperTlsSnapshot,
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
                .or_insert_with(|| (stats, EscaperSnapshot::default()));
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

pub(in crate::stat) fn emit_stats(client: &mut StatsdClient) {
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
    client: &mut StatsdClient,
    stats: &ArcEscaperStats,
    snap: &mut EscaperSnapshot,
) {
    let mut common_tags = StatsdTagGroup::default();
    common_tags.add_escaper_tags(stats.name(), stats.stat_id());
    if let Some(tags) = stats.load_extra_tags() {
        common_tags.add_static_tags(&tags);
    }

    let new_value = stats.get_task_total();
    let diff_value = new_value.wrapping_sub(snap.task_total);
    client
        .count_with_tags(METRIC_NAME_ESCAPER_TASK_TOTAL, diff_value, &common_tags)
        .send();
    snap.task_total = new_value;

    let new_value = stats.connection_attempted();
    let diff_value = new_value.wrapping_sub(snap.conn_attempt);
    client
        .count_with_tags(METRIC_NAME_ESCAPER_CONN_ATTEMPT, diff_value, &common_tags)
        .send();
    snap.conn_attempt = new_value;

    let new_value = stats.connection_established();
    let diff_value = new_value.wrapping_sub(snap.conn_establish);
    client
        .count_with_tags(METRIC_NAME_ESCAPER_CONN_ESTABLISH, diff_value, &common_tags)
        .send();
    snap.conn_establish = new_value;

    if let Some(connect_stats) = stats.tcp_connect_snapshot() {
        emit_tcp_connect_stats(client, connect_stats, &mut snap.tcp_connect, &common_tags);
    }

    if let Some(tls_stats) = stats.tls_snapshot() {
        emit_tls_stats(client, tls_stats, &mut snap.tls, &common_tags);
    }

    if let Some(forbidden_stats) = stats.forbidden_snapshot() {
        emit_forbidden_stats(client, forbidden_stats, &mut snap.forbidden, &common_tags);
    }

    if let Some(tcp_io_stats) = stats.tcp_io_snapshot() {
        emit_tcp_io_to_statsd(client, tcp_io_stats, &mut snap.tcp, &common_tags);
    }

    if let Some(udp_io_stats) = stats.udp_io_snapshot() {
        emit_udp_io_to_statsd(client, udp_io_stats, &mut snap.udp, &common_tags);
    }
}

fn emit_tcp_connect_stats(
    client: &mut StatsdClient,
    stats: EscaperTcpConnectSnapshot,
    snap: &mut EscaperTcpConnectSnapshot,
    common_tags: &StatsdTagGroup,
) {
    macro_rules! emit_optional_field {
        ($field:ident, $name:expr) => {
            let new_value = stats.$field;
            if new_value != 0 || snap.$field != 0 {
                let diff_value = new_value.wrapping_sub(snap.$field);
                client
                    .count_with_tags($name, diff_value, common_tags)
                    .send();
                snap.$field = new_value;
            }
        };
    }

    emit_optional_field!(attempt, METRIC_NAME_ESCAPER_TCP_CONNECT_ATTEMPT);
    emit_optional_field!(establish, METRIC_NAME_ESCAPER_TCP_CONNECT_ESTABLISH);
    emit_optional_field!(success, METRIC_NAME_ESCAPER_TCP_CONNECT_SUCCESS);
    emit_optional_field!(error, METRIC_NAME_ESCAPER_TCP_CONNECT_ERROR);
    emit_optional_field!(timeout, METRIC_NAME_ESCAPER_TCP_CONNECT_TIMEOUT);
}

fn emit_tls_stats(
    client: &mut StatsdClient,
    stats: EscaperTlsSnapshot,
    snap: &mut EscaperTlsSnapshot,
    common_tags: &StatsdTagGroup,
) {
    macro_rules! emit_optional_field {
        ($field:ident, $name:expr) => {
            let new_value = stats.$field;
            if new_value != 0 || snap.$field != 0 {
                let diff_value = new_value.wrapping_sub(snap.$field);
                client
                    .count_with_tags($name, diff_value, common_tags)
                    .send();
                snap.$field = new_value;
            }
        };
    }

    emit_optional_field!(handshake_success, METRIC_NAME_ESCAPER_TLS_HANDSHAKE_SUCCESS);
    emit_optional_field!(handshake_error, METRIC_NAME_ESCAPER_TLS_HANDSHAKE_ERROR);
    emit_optional_field!(handshake_timeout, METRIC_NAME_ESCAPER_TLS_HANDSHAKE_TIMEOUT);
    emit_optional_field!(
        peer_orderly_closure,
        METRIC_NAME_ESCAPER_TLS_PEER_ORDERLY_CLOSURE
    );
    emit_optional_field!(
        peer_abortive_closure,
        METRIC_NAME_ESCAPER_TLS_PEER_ABORTIVE_CLOSURE
    );
}

fn emit_forbidden_stats(
    client: &mut StatsdClient,
    stats: EscaperForbiddenSnapshot,
    snap: &mut EscaperForbiddenSnapshot,
    common_tags: &StatsdTagGroup,
) {
    let new_value = stats.ip_blocked;
    if new_value != 0 || snap.ip_blocked != 0 {
        let diff_value = new_value.wrapping_sub(snap.ip_blocked);
        client
            .count_with_tags(
                METRIC_NAME_ESCAPER_FORBIDDEN_IP_BLOCKED,
                diff_value,
                common_tags,
            )
            .send();
        snap.ip_blocked = new_value;
    }
}

fn emit_tcp_io_to_statsd(
    client: &mut StatsdClient,
    stats: TcpIoSnapshot,
    snap: &mut TcpIoSnapshot,
    common_tags: &StatsdTagGroup,
) {
    if stats.out_bytes == 0 && snap.out_bytes == 0 {
        return;
    }

    macro_rules! emit_field {
        ($field:ident, $name:expr) => {
            let new_value = stats.$field;
            let diff_value = new_value.wrapping_sub(snap.$field);
            client
                .count_with_tags($name, diff_value, common_tags)
                .with_tag(TAG_KEY_TRANSPORT, TRANSPORT_TYPE_TCP)
                .send();
            snap.$field = new_value;
        };
    }

    emit_field!(out_bytes, METRIC_NAME_ESCAPER_IO_OUT_BYTES);
    emit_field!(in_bytes, METRIC_NAME_ESCAPER_IO_IN_BYTES);
}

fn emit_udp_io_to_statsd(
    client: &mut StatsdClient,
    stats: UdpIoSnapshot,
    snap: &mut UdpIoSnapshot,
    common_tags: &StatsdTagGroup,
) {
    if stats.out_packets == 0 && snap.out_packets == 0 {
        return;
    }

    macro_rules! emit_field {
        ($field:ident, $name:expr) => {
            let new_value = stats.$field;
            let diff_value = new_value.wrapping_sub(snap.$field);
            client
                .count_with_tags($name, diff_value, common_tags)
                .with_tag(TAG_KEY_TRANSPORT, TRANSPORT_TYPE_UDP)
                .send();
            snap.$field = new_value;
        };
    }

    emit_field!(out_packets, METRIC_NAME_ESCAPER_IO_OUT_PACKETS);
    emit_field!(out_bytes, METRIC_NAME_ESCAPER_IO_OUT_BYTES);
    emit_field!(in_packets, METRIC_NAME_ESCAPER_IO_IN_PACKETS);
    emit_field!(in_bytes, METRIC_NAME_ESCAPER_IO_IN_BYTES);
}

fn emit_route_stats(
    client: &mut StatsdClient,
    stats: &Arc<RouteEscaperStats>,
    snap: &mut RouteEscaperSnapshot,
) {
    let mut common_tags = StatsdTagGroup::default();
    common_tags.add_escaper_tags(stats.name(), stats.stat_id());

    let stats = stats.snapshot();

    let new_value = stats.request_passed;
    let diff_value = new_value.wrapping_sub(snap.request_passed);
    client
        .count_with_tags(METRIC_NAME_ROUTE_REQUEST_PASSED, diff_value, &common_tags)
        .send();
    snap.request_passed = new_value;

    let new_value = stats.request_failed;
    if new_value != 0 || snap.request_failed != 0 {
        let diff_value = new_value.wrapping_sub(snap.request_failed);
        client
            .count_with_tags(METRIC_NAME_ROUTE_REQUEST_FAILED, diff_value, &common_tags)
            .send();
        snap.request_failed = new_value;
    }
}
