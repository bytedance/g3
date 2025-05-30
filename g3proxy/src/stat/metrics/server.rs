/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::{Arc, Mutex};

use g3_daemon::listen::{ListenSnapshot, ListenStats};
use g3_daemon::metrics::{
    ServerMetricExt, TAG_KEY_TRANSPORT, TRANSPORT_TYPE_TCP, TRANSPORT_TYPE_UDP,
};
use g3_statsd_client::{StatsdClient, StatsdTagGroup};
use g3_types::stats::{GlobalStatsMap, TcpIoSnapshot, UdpIoSnapshot};

use crate::serve::{ArcServerStats, ServerForbiddenSnapshot};
use crate::stat::types::UntrustedTaskStatsSnapshot;

const METRIC_NAME_SERVER_CONN_TOTAL: &str = "server.connection.total";
const METRIC_NAME_SERVER_TASK_TOTAL: &str = "server.task.total";
const METRIC_NAME_SERVER_TASK_ALIVE: &str = "server.task.alive";
const METRIC_NAME_SERVER_FORBIDDEN_AUTH_FAILED: &str = "server.forbidden.auth_failed";
const METRIC_NAME_SERVER_FORBIDDEN_DEST_DENIED: &str = "server.forbidden.dest_denied";
const METRIC_NAME_SERVER_FORBIDDEN_USER_BLOCKED: &str = "server.forbidden.user_blocked";
const METRIC_NAME_SERVER_IO_IN_BYTES: &str = "server.traffic.in.bytes";
const METRIC_NAME_SERVER_IO_IN_PACKETS: &str = "server.traffic.in.packets";
const METRIC_NAME_SERVER_IO_OUT_BYTES: &str = "server.traffic.out.bytes";
const METRIC_NAME_SERVER_IO_OUT_PACKETS: &str = "server.traffic.out.packets";
const METRIC_NAME_SERVER_UNTRUSTED_TASK_TOTAL: &str = "server.task.untrusted_total";
const METRIC_NAME_SERVER_UNTRUSTED_TASK_ALIVE: &str = "server.task.untrusted_alive";
const METRIC_NAME_SERVER_IO_UNTRUSTED_IN_BYTES: &str = "server.traffic.untrusted_in.bytes";

type ServerStatsValue = (ArcServerStats, ServerSnapshot);
type ListenStatsValue = (Arc<ListenStats>, ListenSnapshot);

static SERVER_STATS_MAP: Mutex<GlobalStatsMap<ServerStatsValue>> =
    Mutex::new(GlobalStatsMap::new());
static LISTEN_STATS_MAP: Mutex<GlobalStatsMap<ListenStatsValue>> =
    Mutex::new(GlobalStatsMap::new());

#[derive(Default)]
struct ServerSnapshot {
    conn_total: u64,
    task_total: u64,
    forbidden: ServerForbiddenSnapshot,
    tcp: TcpIoSnapshot,
    udp: UdpIoSnapshot,
    untrusted: UntrustedTaskStatsSnapshot,
}

pub(in crate::stat) fn sync_stats() {
    let mut server_stats_map = SERVER_STATS_MAP.lock().unwrap();
    crate::serve::foreach_server(|_, server| {
        if let Some(stats) = server.get_server_stats() {
            server_stats_map
                .get_or_insert_with(stats.stat_id(), || (stats, ServerSnapshot::default()));
        }
    });
    drop(server_stats_map);

    let mut listen_stats_map = LISTEN_STATS_MAP.lock().unwrap();
    crate::serve::foreach_server(|_, server| {
        let stats = server.get_listen_stats();
        listen_stats_map.get_or_insert_with(stats.stat_id(), || (stats, ListenSnapshot::default()));
    });
    drop(listen_stats_map);
}

pub(in crate::stat) fn emit_stats(client: &mut StatsdClient) {
    let mut server_stats_map = SERVER_STATS_MAP.lock().unwrap();
    server_stats_map.retain(|(stats, snap)| {
        emit_server_stats(client, stats, snap);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
    drop(server_stats_map);

    let mut listen_stats_map = LISTEN_STATS_MAP.lock().unwrap();
    listen_stats_map.retain(|(stats, snap)| {
        g3_daemon::metrics::emit_listen_stats(client, stats, snap);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
}

fn emit_server_stats(client: &mut StatsdClient, stats: &ArcServerStats, snap: &mut ServerSnapshot) {
    let mut common_tags = StatsdTagGroup::default();
    common_tags.add_server_tags(stats.name(), stats.is_online(), stats.stat_id());
    if let Some(tags) = stats.load_extra_tags() {
        common_tags.add_static_tags(&tags);
    }

    let new_value = stats.get_conn_total();
    let diff_value = new_value.wrapping_sub(snap.conn_total);
    client
        .count_with_tags(METRIC_NAME_SERVER_CONN_TOTAL, diff_value, &common_tags)
        .send();
    snap.conn_total = new_value;

    let new_value = stats.get_task_total();
    let diff_value = new_value.wrapping_sub(snap.task_total);
    client
        .count_with_tags(METRIC_NAME_SERVER_TASK_TOTAL, diff_value, &common_tags)
        .send();
    snap.task_total = new_value;

    client
        .gauge_with_tags(
            METRIC_NAME_SERVER_TASK_ALIVE,
            stats.get_alive_count(),
            &common_tags,
        )
        .send();

    emit_forbidden_stats(
        client,
        stats.forbidden_stats(),
        &mut snap.forbidden,
        &common_tags,
    );

    if let Some(tcp_io_stats) = stats.tcp_io_snapshot() {
        emit_tcp_io_to_statsd(client, tcp_io_stats, &mut snap.tcp, &common_tags);
    }

    if let Some(udp_io_stats) = stats.udp_io_snapshot() {
        emit_udp_io_to_statsd(client, udp_io_stats, &mut snap.udp, &common_tags);
    }

    if let Some(untrusted_stats) = stats.untrusted_snapshot() {
        emit_untrusted_stats(client, untrusted_stats, &mut snap.untrusted, &common_tags);
    }
}

fn emit_forbidden_stats(
    client: &mut StatsdClient,
    stats: ServerForbiddenSnapshot,
    snap: &mut ServerForbiddenSnapshot,
    common_tags: &StatsdTagGroup,
) {
    macro_rules! emit_forbid_stats_u64 {
        ($id:ident, $name:expr) => {
            let new_value = stats.$id;
            if new_value != 0 || snap.$id != 0 {
                let diff_value = new_value.wrapping_sub(snap.$id);
                client
                    .count_with_tags($name, diff_value, common_tags)
                    .send();
                snap.$id = new_value;
            }
        };
    }

    emit_forbid_stats_u64!(auth_failed, METRIC_NAME_SERVER_FORBIDDEN_AUTH_FAILED);
    emit_forbid_stats_u64!(dest_denied, METRIC_NAME_SERVER_FORBIDDEN_DEST_DENIED);
    emit_forbid_stats_u64!(user_blocked, METRIC_NAME_SERVER_FORBIDDEN_USER_BLOCKED);
}

fn emit_tcp_io_to_statsd(
    client: &mut StatsdClient,
    stats: TcpIoSnapshot,
    snap: &mut TcpIoSnapshot,
    common_tags: &StatsdTagGroup,
) {
    if stats.in_bytes == 0 && snap.in_bytes == 0 {
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

    emit_field!(in_bytes, METRIC_NAME_SERVER_IO_IN_BYTES);
    emit_field!(out_bytes, METRIC_NAME_SERVER_IO_OUT_BYTES);
}

fn emit_udp_io_to_statsd(
    client: &mut StatsdClient,
    stats: UdpIoSnapshot,
    snap: &mut UdpIoSnapshot,
    common_tags: &StatsdTagGroup,
) {
    if stats.in_packets == 0 && snap.in_packets == 0 {
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

    emit_field!(in_packets, METRIC_NAME_SERVER_IO_IN_PACKETS);
    emit_field!(in_bytes, METRIC_NAME_SERVER_IO_IN_BYTES);
    emit_field!(out_packets, METRIC_NAME_SERVER_IO_OUT_PACKETS);
    emit_field!(out_bytes, METRIC_NAME_SERVER_IO_OUT_BYTES);
}

fn emit_untrusted_stats(
    client: &mut StatsdClient,
    stats: UntrustedTaskStatsSnapshot,
    snap: &mut UntrustedTaskStatsSnapshot,
    common_tags: &StatsdTagGroup,
) {
    let new_value = stats.task_total;
    if new_value == 0 && snap.task_total == 0 {
        return;
    }
    let diff_value = new_value.wrapping_sub(snap.task_total);
    client
        .count_with_tags(
            METRIC_NAME_SERVER_UNTRUSTED_TASK_TOTAL,
            diff_value,
            common_tags,
        )
        .send();
    snap.task_total = new_value;

    client
        .gauge_with_tags(
            METRIC_NAME_SERVER_UNTRUSTED_TASK_ALIVE,
            stats.task_alive,
            common_tags,
        )
        .send();

    let new_value = stats.in_bytes;
    let diff_value = new_value.wrapping_sub(snap.in_bytes);
    client
        .count_with_tags(
            METRIC_NAME_SERVER_IO_UNTRUSTED_IN_BYTES,
            diff_value,
            common_tags,
        )
        .with_tag(TAG_KEY_TRANSPORT, TRANSPORT_TYPE_TCP)
        .send();
    snap.in_bytes = new_value;
}
