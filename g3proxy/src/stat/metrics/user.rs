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

use std::sync::{Arc, Mutex};

use ahash::AHashMap;
use once_cell::sync::Lazy;

use g3_daemon::metrics::{
    MetricTransportType, TAG_KEY_CONNECTION, TAG_KEY_REQUEST, TAG_KEY_SERVER, TAG_KEY_STAT_ID,
    TAG_KEY_TRANSPORT,
};
use g3_statsd_client::{StatsdClient, StatsdTagGroup};
use g3_types::metrics::MetricsName;
use g3_types::stats::{StatId, TcpIoSnapshot, UdpIoSnapshot};

use super::TAG_KEY_ESCAPER;
use super::{MetricUserConnectionType, MetricUserRequestType};
use crate::auth::{
    User, UserForbiddenSnapshot, UserForbiddenStats, UserRequestSnapshot, UserRequestStats,
    UserTrafficSnapshot, UserTrafficStats, UserUpstreamTrafficSnapshot, UserUpstreamTrafficStats,
};
use crate::stat::types::{
    ConnectionSnapshot, ConnectionStats, KeepaliveRequestSnapshot, KeepaliveRequestStats,
    L7ConnectionAliveStats, RequestAliveStats, RequestSnapshot, RequestStats, TrafficSnapshot,
    TrafficStats, UpstreamTrafficSnapshot, UpstreamTrafficStats,
};

const TAG_KEY_USER_GROUP: &str = "user_group";
const TAG_KEY_USER: &str = "user";
const TAG_KEY_USER_TYPE: &str = "user_type";

const METRIC_NAME_FORBIDDEN_AUTH_FAILED: &str = "user.forbidden.auth_failed";
const METRIC_NAME_FORBIDDEN_USER_EXPIRED: &str = "user.forbidden.user_expired";
const METRIC_NAME_FORBIDDEN_USER_BLOCKED: &str = "user.forbidden.user_blocked";
const METRIC_NAME_FORBIDDEN_FULLY_LOADED: &str = "user.forbidden.fully_loaded";
const METRIC_NAME_FORBIDDEN_RATE_LIMITED: &str = "user.forbidden.rate_limited";
const METRIC_NAME_FORBIDDEN_PROTO_BANNED: &str = "user.forbidden.proto_banned";
const METRIC_NAME_FORBIDDEN_SRC_BLOCKED: &str = "user.forbidden.src_blocked";
const METRIC_NAME_FORBIDDEN_DEST_DENIED: &str = "user.forbidden.dest_denied";
const METRIC_NAME_FORBIDDEN_IP_BLOCKED: &str = "user.forbidden.ip_blocked";
const METRIC_NAME_FORBIDDEN_LOG_SKIPPED: &str = "user.forbidden.log_skipped";
const METRIC_NAME_FORBIDDEN_UA_BLOCKED: &str = "user.forbidden.ua_blocked";

pub(super) struct RequestStatsNamesRef<'a> {
    pub(super) connection_total: &'a str,
    pub(super) request_total: &'a str,
    pub(super) request_alive: &'a str,
    pub(super) request_ready: &'a str,
    pub(super) request_reuse: &'a str,
    pub(super) request_renew: &'a str,
    pub(super) l7_connection_alive: &'a str,
}

pub(super) struct TrafficStatsNamesRef<'a> {
    pub(super) in_bytes: &'a str,
    pub(super) in_packets: &'a str,
    pub(super) out_bytes: &'a str,
    pub(super) out_packets: &'a str,
}

const REQUEST_STATS_NAMES: RequestStatsNamesRef<'static> = RequestStatsNamesRef {
    connection_total: "user.connection.total",
    request_total: "user.request.total",
    request_alive: "user.request.alive",
    request_ready: "user.request.ready",
    request_reuse: "user.request.reuse",
    request_renew: "user.request.renew",
    l7_connection_alive: "user.l7.connection.alive",
};

const TRAFFIC_STATS_NAMES: TrafficStatsNamesRef<'static> = TrafficStatsNamesRef {
    in_bytes: "user.traffic.in.bytes",
    in_packets: "user.traffic.in.packets",
    out_bytes: "user.traffic.out.bytes",
    out_packets: "user.traffic.out.packets",
};

const UPSTREAM_TRAFFIC_STATS_NAMES: TrafficStatsNamesRef<'static> = TrafficStatsNamesRef {
    in_bytes: "user.upstream.traffic.in.bytes",
    in_packets: "user.upstream.traffic.in.packets",
    out_bytes: "user.upstream.traffic.out.bytes",
    out_packets: "user.upstream.traffic.out.packets",
};

type ForbiddenStatsValue = (Arc<UserForbiddenStats>, UserForbiddenSnapshot);
type RequestStatsValue = (Arc<UserRequestStats>, UserRequestSnapshot);
type TrafficStatsValue = (Arc<UserTrafficStats>, UserTrafficSnapshot);
type UpstreamTrafficStatsValue = (Arc<UserUpstreamTrafficStats>, UserUpstreamTrafficSnapshot);

static USER_FORBIDDEN_STATS_MAP: Lazy<Mutex<AHashMap<StatId, ForbiddenStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));
static USER_REQUEST_STATS_MAP: Lazy<Mutex<AHashMap<StatId, RequestStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));
static USER_TRAFFIC_STATS_MAP: Lazy<Mutex<AHashMap<StatId, TrafficStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));
static USER_UPSTREAM_TRAFFIC_STATS_MAP: Lazy<Mutex<AHashMap<StatId, UpstreamTrafficStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));

trait UserMetricExt {
    fn add_user_request_tags(
        &mut self,
        user_group: &MetricsName,
        user: &str,
        user_type: &str,
        server: &MetricsName,
        stat_id: StatId,
    );
    fn add_user_upstream_traffic_tags(
        &mut self,
        user_group: &MetricsName,
        user: &str,
        user_type: &str,
        escaper: &MetricsName,
        stat_id: StatId,
    );
}

impl UserMetricExt for StatsdTagGroup {
    fn add_user_request_tags(
        &mut self,
        user_group: &MetricsName,
        user: &str,
        user_type: &str,
        server: &MetricsName,
        stat_id: StatId,
    ) {
        let mut buffer = itoa::Buffer::new();
        let stat_id = buffer.format(stat_id.as_u64());
        self.add_tag(TAG_KEY_USER_GROUP, user_group);
        self.add_tag(TAG_KEY_USER, user);
        self.add_tag(TAG_KEY_USER_TYPE, user_type);
        self.add_tag(TAG_KEY_STAT_ID, stat_id);
        self.add_tag(TAG_KEY_SERVER, server);
    }

    fn add_user_upstream_traffic_tags(
        &mut self,
        user_group: &MetricsName,
        user: &str,
        user_type: &str,
        escaper: &MetricsName,
        stat_id: StatId,
    ) {
        let mut buffer = itoa::Buffer::new();
        let stat_id = buffer.format(stat_id.as_u64());
        self.add_tag(TAG_KEY_USER_GROUP, user_group);
        self.add_tag(TAG_KEY_USER, user);
        self.add_tag(TAG_KEY_USER_TYPE, user_type);
        self.add_tag(TAG_KEY_STAT_ID, stat_id);
        self.add_tag(TAG_KEY_ESCAPER, escaper);
    }
}

pub(in crate::stat) fn sync_stats() {
    let groups = crate::auth::get_all_groups();

    let mut fbd_stats_map = USER_FORBIDDEN_STATS_MAP.lock().unwrap();
    for user_group in groups.iter() {
        user_group.foreach_user(|_, user: &Arc<User>| {
            let all_stats = user.all_forbidden_stats();
            for stats in all_stats {
                let stat_id = stats.stat_id();
                fbd_stats_map
                    .entry(stat_id)
                    .or_insert_with(|| (stats, UserForbiddenSnapshot::default()));
            }
        });
    }
    drop(fbd_stats_map);

    let mut req_stats_map = USER_REQUEST_STATS_MAP.lock().unwrap();
    for user_group in groups.iter() {
        user_group.foreach_user(|_, user: &Arc<User>| {
            let all_stats = user.all_request_stats();
            for stats in all_stats {
                let stat_id = stats.stat_id();
                req_stats_map
                    .entry(stat_id)
                    .or_insert_with(|| (stats, UserRequestSnapshot::default()));
            }
        });
    }
    drop(req_stats_map);

    let mut io_stats_map = USER_TRAFFIC_STATS_MAP.lock().unwrap();
    for user_group in groups.iter() {
        user_group.foreach_user(|_, user: &Arc<User>| {
            let all_stats = user.all_traffic_stats();
            for stats in all_stats {
                let stat_id = stats.stat_id();
                io_stats_map
                    .entry(stat_id)
                    .or_insert_with(|| (stats, UserTrafficSnapshot::default()));
            }
        });
    }
    drop(io_stats_map);

    let mut upstream_io_stats_map = USER_UPSTREAM_TRAFFIC_STATS_MAP.lock().unwrap();
    for user_group in groups.iter() {
        user_group.foreach_user(|_, user: &Arc<User>| {
            let all_stats = user.all_upstream_traffic_stats();
            for stats in all_stats {
                let stat_id = stats.stat_id();
                upstream_io_stats_map
                    .entry(stat_id)
                    .or_insert_with(|| (stats, UserUpstreamTrafficSnapshot::default()));
            }
        });
    }
    drop(upstream_io_stats_map);
}

pub(in crate::stat) fn emit_stats(client: &mut StatsdClient) {
    let mut fbd_stats_map = USER_FORBIDDEN_STATS_MAP.lock().unwrap();
    fbd_stats_map.retain(|_, (stats, snap)| {
        emit_user_forbidden_stats(client, stats, snap);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
    drop(fbd_stats_map);

    let mut req_stats_map = USER_REQUEST_STATS_MAP.lock().unwrap();
    req_stats_map.retain(|_, (stats, snap)| {
        emit_user_request_stats(client, stats, snap, &REQUEST_STATS_NAMES);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
    drop(req_stats_map);

    let mut io_stats_map = USER_TRAFFIC_STATS_MAP.lock().unwrap();
    io_stats_map.retain(|_, (stats, snap)| {
        emit_user_traffic_stats(client, stats, snap, &TRAFFIC_STATS_NAMES);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
    drop(io_stats_map);

    let mut upstream_io_stats_map = USER_UPSTREAM_TRAFFIC_STATS_MAP.lock().unwrap();
    upstream_io_stats_map.retain(|_, (stats, snap)| {
        emit_user_upstream_traffic_stats(client, stats, snap, &UPSTREAM_TRAFFIC_STATS_NAMES);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
    drop(upstream_io_stats_map);
}

fn emit_user_forbidden_stats(
    client: &mut StatsdClient,
    stats: &UserForbiddenStats,
    snap: &mut UserForbiddenSnapshot,
) {
    let mut common_tags = StatsdTagGroup::default();
    common_tags.add_user_request_tags(
        stats.user_group(),
        stats.user(),
        stats.user_type(),
        stats.server(),
        stats.stat_id(),
    );
    if let Some(server_extra_tags) = stats.server_extra_tags() {
        common_tags.add_static_tags(&server_extra_tags);
    }

    let stats = stats.snapshot();

    macro_rules! emit_forbid_stats_u64 {
        ($id:ident, $name:expr) => {
            let new_value = stats.$id;
            if new_value != 0 || snap.$id != 0 {
                let diff_value = new_value.wrapping_sub(snap.$id);
                client
                    .count_with_tags($name, diff_value, &common_tags)
                    .send();
                snap.$id = new_value;
            }
        };
    }

    emit_forbid_stats_u64!(auth_failed, METRIC_NAME_FORBIDDEN_AUTH_FAILED);
    emit_forbid_stats_u64!(user_expired, METRIC_NAME_FORBIDDEN_USER_EXPIRED);
    emit_forbid_stats_u64!(user_blocked, METRIC_NAME_FORBIDDEN_USER_BLOCKED);
    emit_forbid_stats_u64!(fully_loaded, METRIC_NAME_FORBIDDEN_FULLY_LOADED);
    emit_forbid_stats_u64!(rate_limited, METRIC_NAME_FORBIDDEN_RATE_LIMITED);
    emit_forbid_stats_u64!(proto_banned, METRIC_NAME_FORBIDDEN_PROTO_BANNED);
    emit_forbid_stats_u64!(src_blocked, METRIC_NAME_FORBIDDEN_SRC_BLOCKED);
    emit_forbid_stats_u64!(dest_denied, METRIC_NAME_FORBIDDEN_DEST_DENIED);
    emit_forbid_stats_u64!(ip_blocked, METRIC_NAME_FORBIDDEN_IP_BLOCKED);
    emit_forbid_stats_u64!(ua_blocked, METRIC_NAME_FORBIDDEN_UA_BLOCKED);
    emit_forbid_stats_u64!(log_skipped, METRIC_NAME_FORBIDDEN_LOG_SKIPPED);
}

pub(super) fn emit_user_request_stats<'a>(
    client: &'a mut StatsdClient,
    stats: &'a UserRequestStats,
    snap: &'a mut UserRequestSnapshot,
    names: &'a RequestStatsNamesRef<'a>,
) {
    let mut common_tags = StatsdTagGroup::default();
    common_tags.add_user_request_tags(
        stats.user_group(),
        stats.user(),
        stats.user_type(),
        stats.server(),
        stats.stat_id(),
    );
    if let Some(server_extra_tags) = stats.server_extra_tags() {
        common_tags.add_static_tags(&server_extra_tags);
    }

    find_conn_stat(
        &stats.conn_total,
        &mut snap.conn_total,
        |value, conn_type| {
            client
                .count_with_tags(names.connection_total, value, &common_tags)
                .with_tag(TAG_KEY_CONNECTION, conn_type)
                .send();
        },
    );

    find_l7conn_alive_stat(&stats.l7_conn_alive, |value, conn_type| {
        client
            .gauge_with_tags(names.l7_connection_alive, value, &common_tags)
            .with_tag(TAG_KEY_CONNECTION, conn_type)
            .send();
    });

    find_req_stat(&stats.req_total, &mut snap.req_total, |value, req_type| {
        client
            .count_with_tags(names.request_total, value, &common_tags)
            .with_tag(TAG_KEY_REQUEST, req_type)
            .send();
    });

    find_req_alive_stat(&stats.req_alive, |value, req_type| {
        client
            .gauge_with_tags(names.request_alive, value, &common_tags)
            .with_tag(TAG_KEY_REQUEST, req_type)
            .send();
    });

    find_req_stat(&stats.req_ready, &mut snap.req_ready, |value, req_type| {
        client
            .count_with_tags(names.request_ready, value, &common_tags)
            .with_tag(TAG_KEY_REQUEST, req_type)
            .send();
    });

    find_keepalive_req_stat(&stats.req_reuse, &mut snap.req_reuse, |value, req_type| {
        client
            .count_with_tags(names.request_reuse, value, &common_tags)
            .with_tag(TAG_KEY_REQUEST, req_type)
            .send();
    });

    find_keepalive_req_stat(&stats.req_renew, &mut snap.req_renew, |value, req_type| {
        client
            .count_with_tags(names.request_renew, value, &common_tags)
            .with_tag(TAG_KEY_REQUEST, req_type)
            .send();
    });
}

pub(super) fn emit_user_traffic_stats<'a>(
    client: &'a mut StatsdClient,
    stats: &'a UserTrafficStats,
    snap: &'a mut UserTrafficSnapshot,
    names: &'a TrafficStatsNamesRef<'a>,
) {
    let mut common_tags = StatsdTagGroup::default();
    common_tags.add_user_request_tags(
        stats.user_group(),
        stats.user(),
        stats.user_type(),
        stats.server(),
        stats.stat_id(),
    );
    if let Some(server_extra_tags) = stats.server_extra_tags() {
        common_tags.add_static_tags(&server_extra_tags);
    }

    find_io_stat(&stats.io, &mut snap.io, names, |key, value, req_type| {
        client
            .count_with_tags(key, value, &common_tags)
            .with_tag(TAG_KEY_REQUEST, req_type)
            .send();
    });
}

pub(super) fn emit_user_upstream_traffic_stats<'a>(
    client: &'a mut StatsdClient,
    stats: &'a UserUpstreamTrafficStats,
    snap: &'a mut UserUpstreamTrafficSnapshot,
    names: &'a TrafficStatsNamesRef<'a>,
) {
    let mut common_tags = StatsdTagGroup::default();
    common_tags.add_user_upstream_traffic_tags(
        stats.user_group(),
        stats.user(),
        stats.user_type(),
        stats.escaper(),
        stats.stat_id(),
    );
    if let Some(escaper_extra_tags) = stats.escaper_extra_tags() {
        common_tags.add_static_tags(&escaper_extra_tags);
    }

    find_ups_io_stat(&stats.io, &mut snap.io, names, |key, value, trans_type| {
        client
            .count_with_tags(key, value, &common_tags)
            .with_tag(TAG_KEY_TRANSPORT, trans_type)
            .send();
    });
}

fn find_conn_stat<F>(stats: &ConnectionStats, snap: &mut ConnectionSnapshot, mut emit: F)
where
    F: FnMut(u64, MetricUserConnectionType),
{
    let new_value = stats.get_http();
    if new_value != 0 || snap.http != 0 {
        let diff_value = new_value.wrapping_sub(snap.http);
        emit(diff_value, MetricUserConnectionType::Http);
        snap.http = new_value;
    }

    let new_value = stats.get_socks();
    if new_value != 0 || snap.socks != 0 {
        let diff_value = new_value.wrapping_sub(snap.socks);
        emit(diff_value, MetricUserConnectionType::Socks);
        snap.socks = new_value;
    }
}

fn find_l7conn_alive_stat<F>(stats: &L7ConnectionAliveStats, mut emit: F)
where
    F: FnMut(i32, MetricUserConnectionType),
{
    emit(stats.get_http(), MetricUserConnectionType::Http);
}

fn find_req_stat<F>(stats: &RequestStats, snap: &mut RequestSnapshot, mut emit: F)
where
    F: FnMut(u64, MetricUserRequestType),
{
    macro_rules! emit_field {
        ($field:ident, $request:expr) => {
            let new_value = stats.$field();
            if new_value != 0 || snap.$field != 0 {
                let diff_value = new_value.wrapping_sub(snap.$field);
                emit(diff_value, $request);
                snap.$field = new_value;
            }
        };
    }

    emit_field!(http_forward, MetricUserRequestType::HttpForward);
    emit_field!(https_forward, MetricUserRequestType::HttpsForward);
    emit_field!(http_connect, MetricUserRequestType::HttpConnect);
    emit_field!(ftp_over_http, MetricUserRequestType::FtpOverHttp);
    emit_field!(socks_tcp_connect, MetricUserRequestType::SocksTcpConnect);
    emit_field!(socks_udp_connect, MetricUserRequestType::SocksUdpConnect);
    emit_field!(
        socks_udp_associate,
        MetricUserRequestType::SocksUdpAssociate
    );
}

fn find_req_alive_stat<F>(stats: &RequestAliveStats, mut emit: F)
where
    F: FnMut(i32, MetricUserRequestType),
{
    emit(stats.http_forward(), MetricUserRequestType::HttpForward);
    emit(stats.https_forward(), MetricUserRequestType::HttpsForward);
    emit(stats.http_connect(), MetricUserRequestType::HttpConnect);
    emit(stats.ftp_over_http(), MetricUserRequestType::FtpOverHttp);
    emit(
        stats.socks_tcp_connect(),
        MetricUserRequestType::SocksTcpConnect,
    );
    emit(
        stats.socks_udp_connect(),
        MetricUserRequestType::SocksUdpConnect,
    );
    emit(
        stats.socks_udp_associate(),
        MetricUserRequestType::SocksUdpAssociate,
    );
}

fn find_keepalive_req_stat<F>(
    stats: &KeepaliveRequestStats,
    snap: &mut KeepaliveRequestSnapshot,
    mut emit: F,
) where
    F: FnMut(u64, MetricUserRequestType),
{
    macro_rules! emit_field {
        ($field:ident, $request:expr) => {
            let new_value = stats.$field();
            if new_value != 0 || snap.$field != 0 {
                let diff_value = new_value.wrapping_sub(snap.$field);
                emit(diff_value, $request);
                snap.$field = new_value;
            }
        };
    }

    emit_field!(http_forward, MetricUserRequestType::HttpForward);
    emit_field!(https_forward, MetricUserRequestType::HttpsForward);
}

fn find_io_stat<'a, F>(
    stats: &'a TrafficStats,
    snap: &'a mut TrafficSnapshot,
    names: &'a TrafficStatsNamesRef<'a>,
    mut emit: F,
) where
    F: FnMut(&'a str, u64, MetricUserRequestType),
{
    macro_rules! emit_tcp_field {
        ($field:ident, $request:expr) => {
            find_tcp_io_stat(
                stats.$field.snapshot(),
                &mut snap.$field,
                names,
                $request,
                &mut emit,
            );
        };
    }

    emit_tcp_field!(http_forward, MetricUserRequestType::HttpForward);
    emit_tcp_field!(https_forward, MetricUserRequestType::HttpsForward);
    emit_tcp_field!(http_connect, MetricUserRequestType::HttpConnect);
    emit_tcp_field!(ftp_over_http, MetricUserRequestType::FtpOverHttp);
    emit_tcp_field!(socks_tcp_connect, MetricUserRequestType::SocksTcpConnect);

    macro_rules! emit_udp_field {
        ($field:ident, $request:expr) => {
            find_udp_io_stat(
                stats.$field.snapshot(),
                &mut snap.$field,
                names,
                $request,
                &mut emit,
            );
        };
    }

    emit_udp_field!(socks_udp_connect, MetricUserRequestType::SocksUdpConnect);
    emit_udp_field!(
        socks_udp_associate,
        MetricUserRequestType::SocksUdpAssociate
    );
}

fn find_tcp_io_stat<'a, F>(
    stats: TcpIoSnapshot,
    snap: &'a mut TcpIoSnapshot,
    names: &'a TrafficStatsNamesRef<'a>,
    req_type: MetricUserRequestType,
    mut emit: F,
) where
    F: FnMut(&'a str, u64, MetricUserRequestType),
{
    if stats.in_bytes == 0 && snap.in_bytes == 0 {
        return;
    }

    macro_rules! emit_field {
        ($field:ident) => {
            let new_value = stats.$field;
            let diff_value = new_value.wrapping_sub(snap.$field);
            emit(names.$field, diff_value, req_type);
            snap.$field = new_value;
        };
    }

    emit_field!(in_bytes);
    emit_field!(out_bytes);
}

fn find_udp_io_stat<'a, F>(
    stats: UdpIoSnapshot,
    snap: &'a mut UdpIoSnapshot,
    names: &'a TrafficStatsNamesRef<'a>,
    req_type: MetricUserRequestType,
    mut emit: F,
) where
    F: FnMut(&'a str, u64, MetricUserRequestType),
{
    if stats.in_packets == 0 && snap.in_packets == 0 {
        return;
    }

    macro_rules! emit_field {
        ($field:ident) => {
            let new_value = stats.$field;
            let diff_value = new_value.wrapping_sub(snap.$field);
            emit(names.$field, diff_value, req_type);
            snap.$field = new_value;
        };
    }

    emit_field!(in_packets);
    emit_field!(in_bytes);
    emit_field!(out_packets);
    emit_field!(out_bytes);
}

fn find_ups_io_stat<'a, F>(
    stats: &UpstreamTrafficStats,
    snap: &'a mut UpstreamTrafficSnapshot,
    names: &'a TrafficStatsNamesRef<'a>,
    mut emit: F,
) where
    F: FnMut(&'a str, u64, MetricTransportType),
{
    find_ups_tcp_io_stat(stats.tcp.snapshot(), &mut snap.tcp, names, &mut emit);
    find_ups_udp_io_stat(stats.udp.snapshot(), &mut snap.udp, names, &mut emit);
}

fn find_ups_tcp_io_stat<'a, F>(
    stats: TcpIoSnapshot,
    snap: &'a mut TcpIoSnapshot,
    names: &'a TrafficStatsNamesRef<'a>,
    mut emit: F,
) where
    F: FnMut(&'a str, u64, MetricTransportType),
{
    if stats.out_bytes == 0 && snap.out_bytes == 0 {
        return;
    }

    macro_rules! emit_field {
        ($field:ident) => {
            let new_value = stats.$field;
            let diff_value = new_value.wrapping_sub(snap.$field);
            emit(names.$field, diff_value, MetricTransportType::Tcp);
            snap.$field = new_value;
        };
    }

    emit_field!(out_bytes);
    emit_field!(in_bytes);
}

fn find_ups_udp_io_stat<'a, F>(
    stats: UdpIoSnapshot,
    snap: &'a mut UdpIoSnapshot,
    names: &'a TrafficStatsNamesRef<'a>,
    mut emit: F,
) where
    F: FnMut(&'a str, u64, MetricTransportType),
{
    if stats.out_packets == 0 && snap.out_packets == 0 {
        return;
    }

    macro_rules! emit_field {
        ($field:ident) => {
            let new_value = stats.$field;
            let diff_value = new_value.wrapping_sub(snap.$field);
            emit(names.$field, diff_value, MetricTransportType::Udp);
            snap.$field = new_value;
        };
    }

    emit_field!(out_packets);
    emit_field!(out_bytes);
    emit_field!(in_packets);
    emit_field!(in_bytes);
}
