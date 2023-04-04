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

use g3_daemon::metric::{
    MetricTransportType, TAG_KEY_CONNECTION, TAG_KEY_REQUEST, TAG_KEY_STAT_ID, TAG_KEY_TRANSPORT,
};
use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_types::stats::{StatId, TcpIoSnapshot, UdpIoSnapshot};

use super::{MetricUserConnectionType, MetricUserRequestType};
use super::{TAG_KEY_ESCAPER, TAG_KEY_SERVER};
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

trait UserMetricExt<'m> {
    fn add_user_request_tags(
        self,
        user_group: &'m MetricsName,
        user: &'m str,
        user_type: &'m str,
        server: &'m str,
        stat_id: &'m str,
    ) -> Self;
    fn add_user_traffic_tags(
        self,
        user_group: &'m MetricsName,
        user: &'m str,
        user_type: &'m str,
        server: &'m str,
        req_type: MetricUserRequestType,
        stat_id: &'m str,
    ) -> Self;
    fn add_user_upstream_traffic_tags(
        self,
        user_group: &'m MetricsName,
        user: &'m str,
        user_type: &'m str,
        escaper: &'m str,
        trans_type: MetricTransportType,
        stat_id: &'m str,
    ) -> Self;
    fn add_extra_tags(self, tags: &'m Option<Arc<StaticMetricsTags>>) -> Self;
}

impl<'m, 'c, T> UserMetricExt<'m> for MetricBuilder<'m, 'c, T>
where
    T: Metric + From<String>,
{
    fn add_user_request_tags(
        self,
        user_group: &'m MetricsName,
        user: &'m str,
        user_type: &'m str,
        server: &'m str,
        stat_id: &'m str,
    ) -> Self {
        self.with_tag(TAG_KEY_USER_GROUP, user_group.as_str())
            .with_tag(TAG_KEY_USER, user)
            .with_tag(TAG_KEY_USER_TYPE, user_type)
            .with_tag(TAG_KEY_STAT_ID, stat_id)
            .with_tag(TAG_KEY_SERVER, server)
    }

    fn add_user_traffic_tags(
        self,
        user_group: &'m MetricsName,
        user: &'m str,
        user_type: &'m str,
        server: &'m str,
        req_type: MetricUserRequestType,
        stat_id: &'m str,
    ) -> Self {
        self.add_user_request_tags(user_group, user, user_type, server, stat_id)
            .with_tag(TAG_KEY_REQUEST, req_type.as_str())
    }

    fn add_user_upstream_traffic_tags(
        self,
        user_group: &'m MetricsName,
        user: &'m str,
        user_type: &'m str,
        escaper: &'m str,
        trans_type: MetricTransportType,
        stat_id: &'m str,
    ) -> Self {
        self.with_tag(TAG_KEY_USER_GROUP, user_group.as_str())
            .with_tag(TAG_KEY_USER, user)
            .with_tag(TAG_KEY_USER_TYPE, user_type)
            .with_tag(TAG_KEY_STAT_ID, stat_id)
            .with_tag(TAG_KEY_ESCAPER, escaper)
            .with_tag(TAG_KEY_TRANSPORT, trans_type.as_str())
    }

    fn add_extra_tags(mut self, tags: &'m Option<Arc<StaticMetricsTags>>) -> Self {
        if let Some(tags) = tags {
            for (k, v) in tags.iter() {
                self = self.with_tag(k.as_str(), v.as_str());
            }
        }
        self
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

pub(in crate::stat) fn emit_stats(client: &StatsdClient) {
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
    client: &StatsdClient,
    stats: &UserForbiddenStats,
    snap: &mut UserForbiddenSnapshot,
) {
    let user_group = stats.user_group();
    let user = stats.user();
    let user_type = stats.user_type();
    let server = stats.server();
    let mut buffer = itoa::Buffer::new();
    let stat_id = buffer.format(stats.stat_id().as_u64());

    let server_extra_tags = stats.server_extra_tags();

    let stats = stats.snapshot();

    macro_rules! emit_forbid_stats_u64 {
        ($id:ident, $name:expr) => {
            let new_value = stats.$id;
            if new_value != 0 || snap.$id != 0 {
                let diff_value =
                    i64::try_from(new_value.wrapping_sub(snap.$id)).unwrap_or(i64::MAX);
                client
                    .count_with_tags($name, diff_value)
                    .add_user_request_tags(user_group, user, user_type, server, stat_id)
                    .add_extra_tags(&server_extra_tags)
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
    emit_forbid_stats_u64!(dest_denied, METRIC_NAME_FORBIDDEN_DEST_DENIED);
    emit_forbid_stats_u64!(ip_blocked, METRIC_NAME_FORBIDDEN_IP_BLOCKED);
    emit_forbid_stats_u64!(ua_blocked, METRIC_NAME_FORBIDDEN_UA_BLOCKED);
    emit_forbid_stats_u64!(log_skipped, METRIC_NAME_FORBIDDEN_LOG_SKIPPED);
}

pub(super) fn emit_user_request_stats<'a>(
    client: &'a StatsdClient,
    stats: &'a UserRequestStats,
    snap: &'a mut UserRequestSnapshot,
    names: &'a RequestStatsNamesRef<'a>,
) {
    let user_group = stats.user_group();
    let user = stats.user();
    let user_type = stats.user_type();
    let server = stats.server();
    let mut buffer = itoa::Buffer::new();
    let stat_id = buffer.format(stats.stat_id().as_u64());

    let server_extra_tags = stats.server_extra_tags();

    find_conn_stat(
        &stats.conn_total,
        &mut snap.conn_total,
        |value, conn_type| {
            client
                .count_with_tags(names.connection_total, value)
                .add_user_request_tags(user_group, user, user_type, server, stat_id)
                .add_extra_tags(&server_extra_tags)
                .with_tag(TAG_KEY_CONNECTION, conn_type.as_str())
                .send();
        },
    );

    find_l7conn_alive_stat(&stats.l7_conn_alive, |value, conn_type| {
        client
            .gauge_with_tags(names.l7_connection_alive, value as f64)
            .add_user_request_tags(user_group, user, user_type, server, stat_id)
            .add_extra_tags(&server_extra_tags)
            .with_tag(TAG_KEY_CONNECTION, conn_type.as_str())
            .send();
    });

    find_req_stat(&stats.req_total, &mut snap.req_total, |value, req_type| {
        client
            .count_with_tags(names.request_total, value)
            .add_user_request_tags(user_group, user, user_type, server, stat_id)
            .add_extra_tags(&server_extra_tags)
            .with_tag(TAG_KEY_REQUEST, req_type.as_str())
            .send();
    });

    find_req_alive_stat(&stats.req_alive, |value, req_type| {
        client
            .gauge_with_tags(names.request_alive, value as f64)
            .add_user_request_tags(user_group, user, user_type, server, stat_id)
            .add_extra_tags(&server_extra_tags)
            .with_tag(TAG_KEY_REQUEST, req_type.as_str())
            .send();
    });

    find_req_stat(&stats.req_ready, &mut snap.req_ready, |value, req_type| {
        client
            .count_with_tags(names.request_ready, value)
            .add_user_request_tags(user_group, user, user_type, server, stat_id)
            .add_extra_tags(&server_extra_tags)
            .with_tag(TAG_KEY_REQUEST, req_type.as_str())
            .send();
    });

    find_keepalive_req_stat(&stats.req_reuse, &mut snap.req_reuse, |value, req_type| {
        client
            .count_with_tags(names.request_reuse, value)
            .add_user_request_tags(user_group, user, user_type, server, stat_id)
            .add_extra_tags(&server_extra_tags)
            .with_tag(TAG_KEY_REQUEST, req_type.as_str())
            .send();
    });

    find_keepalive_req_stat(&stats.req_renew, &mut snap.req_renew, |value, req_type| {
        client
            .count_with_tags(names.request_renew, value)
            .add_user_request_tags(user_group, user, user_type, server, stat_id)
            .add_extra_tags(&server_extra_tags)
            .with_tag(TAG_KEY_REQUEST, req_type.as_str())
            .send();
    });
}

pub(super) fn emit_user_traffic_stats<'a>(
    client: &'a StatsdClient,
    stats: &'a UserTrafficStats,
    snap: &'a mut UserTrafficSnapshot,
    names: &'a TrafficStatsNamesRef<'a>,
) {
    let user_group = stats.user_group();
    let user = stats.user();
    let user_type = stats.user_type();
    let server = stats.server();
    let mut buffer = itoa::Buffer::new();
    let stat_id = buffer.format(stats.stat_id().as_u64());

    let server_extra_tags = stats.server_extra_tags();

    find_io_stat(&stats.io, &mut snap.io, names, |key, value, req_type| {
        client
            .count_with_tags(key, value)
            .add_user_traffic_tags(user_group, user, user_type, server, req_type, stat_id)
            .add_extra_tags(&server_extra_tags)
            .send();
    });
}

pub(super) fn emit_user_upstream_traffic_stats<'a>(
    client: &'a StatsdClient,
    stats: &'a UserUpstreamTrafficStats,
    snap: &'a mut UserUpstreamTrafficSnapshot,
    names: &'a TrafficStatsNamesRef<'a>,
) {
    let user_group = stats.user_group();
    let user = stats.user();
    let user_type = stats.user_type();
    let escaper = stats.escaper();
    let mut buffer = itoa::Buffer::new();
    let stat_id = buffer.format(stats.stat_id().as_u64());

    let escaper_extra_tags = stats.escaper_extra_tags();

    find_ups_io_stat(&stats.io, &mut snap.io, names, |key, value, trans_type| {
        client
            .count_with_tags(key, value)
            .add_user_upstream_traffic_tags(
                user_group, user, user_type, escaper, trans_type, stat_id,
            )
            .add_extra_tags(&escaper_extra_tags)
            .send();
    });
}

fn find_conn_stat<F>(stats: &ConnectionStats, snap: &mut ConnectionSnapshot, emit: F)
where
    F: Fn(i64, MetricUserConnectionType),
{
    let new_value = stats.get_http();
    if new_value != 0 || snap.http != 0 {
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.http)).unwrap_or(i64::MAX);
        emit(diff_value, MetricUserConnectionType::Http);
        snap.http = new_value;
    }

    let new_value = stats.get_socks();
    if new_value != 0 || snap.socks != 0 {
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.socks)).unwrap_or(i64::MAX);
        emit(diff_value, MetricUserConnectionType::Socks);
        snap.socks = new_value;
    }
}

fn find_l7conn_alive_stat<F>(stats: &L7ConnectionAliveStats, emit: F)
where
    F: Fn(i32, MetricUserConnectionType),
{
    emit(stats.get_http(), MetricUserConnectionType::Http);
}

fn find_req_stat<F>(stats: &RequestStats, snap: &mut RequestSnapshot, emit: F)
where
    F: Fn(i64, MetricUserRequestType),
{
    let new_value = stats.get_http_forward();
    if new_value != 0 || snap.http_forward != 0 {
        let diff_value =
            i64::try_from(new_value.wrapping_sub(snap.http_forward)).unwrap_or(i64::MAX);
        emit(diff_value, MetricUserRequestType::HttpForward);
        snap.http_forward = new_value;
    }

    let new_value = stats.get_https_forward();
    if new_value != 0 || snap.https_forward != 0 {
        let diff_value =
            i64::try_from(new_value.wrapping_sub(snap.https_forward)).unwrap_or(i64::MAX);
        emit(diff_value, MetricUserRequestType::HttpsForward);
        snap.https_forward = new_value;
    }

    let new_value = stats.get_http_connect();
    if new_value != 0 || snap.http_connect != 0 {
        let diff_value =
            i64::try_from(new_value.wrapping_sub(snap.http_connect)).unwrap_or(i64::MAX);
        emit(diff_value, MetricUserRequestType::HttpConnect);
        snap.http_connect = new_value;
    }

    let new_value = stats.get_ftp_over_http();
    if new_value != 0 || snap.ftp_over_http != 0 {
        let diff_value =
            i64::try_from(new_value.wrapping_sub(snap.ftp_over_http)).unwrap_or(i64::MAX);
        emit(diff_value, MetricUserRequestType::FtpOverHttp);
        snap.ftp_over_http = new_value;
    }

    let new_value = stats.get_socks_tcp_connect();
    if new_value != 0 || snap.socks_tcp_connect != 0 {
        let diff_value =
            i64::try_from(new_value.wrapping_sub(snap.socks_tcp_connect)).unwrap_or(i64::MAX);
        emit(diff_value, MetricUserRequestType::SocksTcpConnect);
        snap.socks_tcp_connect = new_value;
    }

    let new_value = stats.get_socks_udp_connect();
    if new_value != 0 || snap.socks_udp_connect != 0 {
        let diff_value =
            i64::try_from(new_value.wrapping_sub(snap.socks_udp_connect)).unwrap_or(i64::MAX);
        emit(diff_value, MetricUserRequestType::SocksUdpConnect);
        snap.socks_udp_connect = new_value;
    }

    let new_value = stats.get_socks_udp_associate();
    if new_value != 0 || snap.socks_udp_associate != 0 {
        let diff_value =
            i64::try_from(new_value.wrapping_sub(snap.socks_udp_associate)).unwrap_or(i64::MAX);
        emit(diff_value, MetricUserRequestType::SocksUdpAssociate);
        snap.socks_udp_associate = new_value;
    }
}

fn find_req_alive_stat<F>(stats: &RequestAliveStats, emit: F)
where
    F: Fn(i32, MetricUserRequestType),
{
    emit(stats.get_http_forward(), MetricUserRequestType::HttpForward);
    emit(
        stats.get_https_forward(),
        MetricUserRequestType::HttpsForward,
    );
    emit(stats.get_http_connect(), MetricUserRequestType::HttpConnect);
    emit(
        stats.get_ftp_over_http(),
        MetricUserRequestType::FtpOverHttp,
    );
    emit(
        stats.get_socks_tcp_connect(),
        MetricUserRequestType::SocksTcpConnect,
    );
    emit(
        stats.get_socks_udp_connect(),
        MetricUserRequestType::SocksUdpConnect,
    );
    emit(
        stats.get_socks_udp_associate(),
        MetricUserRequestType::SocksUdpAssociate,
    );
}

fn find_keepalive_req_stat<F>(
    stats: &KeepaliveRequestStats,
    snap: &mut KeepaliveRequestSnapshot,
    emit: F,
) where
    F: Fn(i64, MetricUserRequestType),
{
    let new_value = stats.get_http_forward();
    if new_value != 0 || snap.http_forward != 0 {
        let diff_value =
            i64::try_from(new_value.wrapping_sub(snap.http_forward)).unwrap_or(i64::MAX);
        emit(diff_value, MetricUserRequestType::HttpForward);
        snap.http_forward = new_value;
    }

    let new_value = stats.get_https_forward();
    if new_value != 0 || snap.https_forward != 0 {
        let diff_value =
            i64::try_from(new_value.wrapping_sub(snap.https_forward)).unwrap_or(i64::MAX);
        emit(diff_value, MetricUserRequestType::HttpsForward);
        snap.https_forward = new_value;
    }
}

fn find_io_stat<'a, F>(
    stats: &'a TrafficStats,
    snap: &'a mut TrafficSnapshot,
    names: &'a TrafficStatsNamesRef<'a>,
    emit: F,
) where
    F: Fn(&'a str, i64, MetricUserRequestType),
{
    find_tcp_io_stat(
        stats.http_forward.snapshot(),
        &mut snap.http_forward,
        names,
        MetricUserRequestType::HttpForward,
        &emit,
    );

    find_tcp_io_stat(
        stats.https_forward.snapshot(),
        &mut snap.https_forward,
        names,
        MetricUserRequestType::HttpsForward,
        &emit,
    );

    find_tcp_io_stat(
        stats.http_connect.snapshot(),
        &mut snap.http_connect,
        names,
        MetricUserRequestType::HttpConnect,
        &emit,
    );

    find_tcp_io_stat(
        stats.ftp_over_http.snapshot(),
        &mut snap.ftp_over_http,
        names,
        MetricUserRequestType::FtpOverHttp,
        &emit,
    );

    find_tcp_io_stat(
        stats.socks_tcp_connect.snapshot(),
        &mut snap.socks_tcp_connect,
        names,
        MetricUserRequestType::SocksTcpConnect,
        &emit,
    );

    find_udp_io_stat(
        stats.socks_udp_connect.snapshot(),
        &mut snap.socks_udp_connect,
        names,
        MetricUserRequestType::SocksUdpConnect,
        &emit,
    );

    find_udp_io_stat(
        stats.socks_udp_associate.snapshot(),
        &mut snap.socks_udp_associate,
        names,
        MetricUserRequestType::SocksUdpAssociate,
        &emit,
    );
}

fn find_tcp_io_stat<'a, F>(
    stats: TcpIoSnapshot,
    snap: &'a mut TcpIoSnapshot,
    names: &'a TrafficStatsNamesRef<'a>,
    req_type: MetricUserRequestType,
    emit: &F,
) where
    F: Fn(&'a str, i64, MetricUserRequestType),
{
    let new_value = stats.in_bytes;
    if new_value != 0 || snap.in_bytes != 0 {
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.in_bytes)).unwrap_or(i64::MAX);
        emit(names.in_bytes, diff_value, req_type);
        snap.in_bytes = new_value;

        let new_value = stats.out_bytes;
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.out_bytes)).unwrap_or(i64::MAX);
        emit(names.out_bytes, diff_value, req_type);
        snap.out_bytes = new_value;
    }
}

fn find_udp_io_stat<'a, F>(
    stats: UdpIoSnapshot,
    snap: &'a mut UdpIoSnapshot,
    names: &'a TrafficStatsNamesRef<'a>,
    req_type: MetricUserRequestType,
    emit: &F,
) where
    F: Fn(&'a str, i64, MetricUserRequestType),
{
    let new_value = stats.in_packets;
    if new_value != 0 || snap.in_packets != 0 {
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.in_packets)).unwrap_or(i64::MAX);
        emit(names.in_packets, diff_value, req_type);
        snap.in_packets = new_value;

        let new_value = stats.in_bytes;
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.in_bytes)).unwrap_or(i64::MAX);
        emit(names.in_bytes, diff_value, req_type);
        snap.in_bytes = new_value;

        let new_value = stats.out_packets;
        let diff_value =
            i64::try_from(new_value.wrapping_sub(snap.out_packets)).unwrap_or(i64::MAX);
        emit(names.out_packets, diff_value, req_type);
        snap.out_packets = new_value;

        let new_value = stats.out_bytes;
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.out_bytes)).unwrap_or(i64::MAX);
        emit(names.out_bytes, diff_value, req_type);
        snap.out_bytes = new_value;
    }
}

fn find_ups_io_stat<'a, F>(
    stats: &UpstreamTrafficStats,
    snap: &'a mut UpstreamTrafficSnapshot,
    names: &'a TrafficStatsNamesRef<'a>,
    emit: F,
) where
    F: Fn(&'a str, i64, MetricTransportType),
{
    find_ups_tcp_io_stat(stats.tcp.snapshot(), &mut snap.tcp, names, &emit);
    find_ups_udp_io_stat(stats.udp.snapshot(), &mut snap.udp, names, &emit);
}

fn find_ups_tcp_io_stat<'a, F>(
    stats: TcpIoSnapshot,
    snap: &'a mut TcpIoSnapshot,
    names: &'a TrafficStatsNamesRef<'a>,
    emit: &F,
) where
    F: Fn(&'a str, i64, MetricTransportType),
{
    let trans_type = MetricTransportType::Tcp;

    let new_value = stats.in_bytes;
    if new_value != 0 || snap.in_bytes != 0 {
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.in_bytes)).unwrap_or(i64::MAX);
        emit(names.in_bytes, diff_value, trans_type);
        snap.in_bytes = new_value;

        let new_value = stats.out_bytes;
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.out_bytes)).unwrap_or(i64::MAX);
        emit(names.out_bytes, diff_value, trans_type);
        snap.out_bytes = new_value;
    }
}

fn find_ups_udp_io_stat<'a, F>(
    stats: UdpIoSnapshot,
    snap: &'a mut UdpIoSnapshot,
    names: &'a TrafficStatsNamesRef<'a>,
    emit: &F,
) where
    F: Fn(&'a str, i64, MetricTransportType),
{
    let trans_type = MetricTransportType::Udp;

    let new_value = stats.in_packets;
    if new_value != 0 || snap.in_packets != 0 {
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.in_packets)).unwrap_or(i64::MAX);
        emit(names.in_packets, diff_value, trans_type);
        snap.in_packets = new_value;

        let new_value = stats.in_bytes;
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.in_bytes)).unwrap_or(i64::MAX);
        emit(names.in_bytes, diff_value, trans_type);
        snap.in_bytes = new_value;

        let new_value = stats.out_packets;
        let diff_value =
            i64::try_from(new_value.wrapping_sub(snap.out_packets)).unwrap_or(i64::MAX);
        emit(names.out_packets, diff_value, trans_type);
        snap.out_packets = new_value;

        let new_value = stats.out_bytes;
        let diff_value = i64::try_from(new_value.wrapping_sub(snap.out_bytes)).unwrap_or(i64::MAX);
        emit(names.out_bytes, diff_value, trans_type);
        snap.out_bytes = new_value;
    }
}
