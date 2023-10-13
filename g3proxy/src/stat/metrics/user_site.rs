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

use std::hash::Hash;
use std::sync::{Arc, Mutex};

use ahash::AHashMap;
use cadence::StatsdClient;
use once_cell::sync::Lazy;

use g3_types::metrics::MetricsName;
use g3_types::stats::StatId;

use super::{RequestStatsNamesRef, TrafficStatsNamesRef};
use crate::auth::{
    UserRequestSnapshot, UserRequestStats, UserTrafficSnapshot, UserTrafficStats,
    UserUpstreamTrafficSnapshot, UserUpstreamTrafficStats,
};

static STORE_REQUEST_STATS_MAP: Lazy<Mutex<AHashMap<StatId, RequestStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));
static STORE_TRAFFIC_STATS_MAP: Lazy<Mutex<AHashMap<StatId, TrafficStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));
static STORE_UPSTREAM_TRAFFIC_STATS_MAP: Lazy<Mutex<AHashMap<StatId, UpstreamTrafficStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));

static USER_SITE_REQUEST_STATS_MAP: Lazy<Mutex<AHashMap<StatId, RequestStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));
static USER_SITE_TRAFFIC_STATS_MAP: Lazy<Mutex<AHashMap<StatId, TrafficStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));
static USER_SITE_UPSTREAM_TRAFFIC_STATS_MAP: Lazy<
    Mutex<AHashMap<StatId, UpstreamTrafficStatsValue>>,
> = Lazy::new(|| Mutex::new(AHashMap::new()));

struct RequestStatsNames {
    connection_total: String,
    request_total: String,
    request_alive: String,
    request_ready: String,
    request_reuse: String,
    request_renew: String,
    l7_connection_alive: String,
}

impl RequestStatsNames {
    fn new(site_id: &MetricsName) -> Self {
        RequestStatsNames {
            connection_total: format!("user.site.{site_id}.connection.total"),
            request_total: format!("user.site.{site_id}.request.total"),
            request_alive: format!("user.site.{site_id}.request.alive"),
            request_ready: format!("user.site.{site_id}.request.ready"),
            request_reuse: format!("user.site.{site_id}.request.reuse"),
            request_renew: format!("user.site.{site_id}.request.renew"),
            l7_connection_alive: format!("user.site.{site_id}.l7.connection.alive"),
        }
    }
}

struct TrafficStatsNames {
    in_bytes: String,
    in_packets: String,
    out_bytes: String,
    out_packets: String,
}

impl TrafficStatsNames {
    fn new_for_client(site_id: &MetricsName) -> Self {
        TrafficStatsNames {
            in_bytes: format!("user.site.{site_id}.traffic.in.bytes"),
            in_packets: format!("user.site.{site_id}.traffic.in.packets"),
            out_bytes: format!("user.site.{site_id}.traffic.out.bytes"),
            out_packets: format!("user.site.{site_id}.traffic.out.packets"),
        }
    }

    fn new_for_upstream(site_id: &MetricsName) -> Self {
        TrafficStatsNames {
            in_bytes: format!("user.site.{site_id}.upstream.traffic.in.bytes"),
            in_packets: format!("user.site.{site_id}.upstream.traffic.in.packets"),
            out_bytes: format!("user.site.{site_id}.upstream.traffic.out.bytes"),
            out_packets: format!("user.site.{site_id}.upstream.traffic.out.packets"),
        }
    }
}

struct RequestStatsValue {
    stats: Arc<UserRequestStats>,
    snap: UserRequestSnapshot,
    names: RequestStatsNames,
}

impl RequestStatsValue {
    fn new(stats: Arc<UserRequestStats>, site_id: &MetricsName) -> Self {
        RequestStatsValue {
            stats,
            snap: Default::default(),
            names: RequestStatsNames::new(site_id),
        }
    }
}

struct TrafficStatsValue {
    stats: Arc<UserTrafficStats>,
    snap: UserTrafficSnapshot,
    names: TrafficStatsNames,
}

impl TrafficStatsValue {
    fn new(stats: Arc<UserTrafficStats>, site_id: &MetricsName) -> Self {
        TrafficStatsValue {
            stats,
            snap: Default::default(),
            names: TrafficStatsNames::new_for_client(site_id),
        }
    }
}

struct UpstreamTrafficStatsValue {
    stats: Arc<UserUpstreamTrafficStats>,
    snap: UserUpstreamTrafficSnapshot,
    tags: TrafficStatsNames,
}

impl UpstreamTrafficStatsValue {
    fn new(stats: Arc<UserUpstreamTrafficStats>, site_id: &MetricsName) -> Self {
        UpstreamTrafficStatsValue {
            stats,
            snap: Default::default(),
            tags: TrafficStatsNames::new_for_upstream(site_id),
        }
    }
}

fn move_ht<K, V>(in_ht_lock: &Mutex<AHashMap<K, V>>, out_ht_lock: &Mutex<AHashMap<K, V>>)
where
    K: Hash + Eq,
{
    let mut tmp_req_map = AHashMap::new();
    let mut in_req_map = in_ht_lock.lock().unwrap();
    for (k, v) in in_req_map.drain() {
        tmp_req_map.insert(k, v);
    }
    drop(in_req_map); // drop early

    if !tmp_req_map.is_empty() {
        let mut out_req_map = out_ht_lock.lock().unwrap();
        for (k, v) in tmp_req_map.drain() {
            out_req_map.insert(k, v);
        }
    }
}

pub(crate) fn push_request_stats(stats: Arc<UserRequestStats>, site_id: &MetricsName) {
    let k = stats.stat_id();
    let v = RequestStatsValue::new(stats, site_id);
    let mut ht = STORE_REQUEST_STATS_MAP.lock().unwrap();
    ht.insert(k, v);
}

pub(crate) fn push_traffic_stats(stats: Arc<UserTrafficStats>, site_id: &MetricsName) {
    let k = stats.stat_id();
    let v = TrafficStatsValue::new(stats, site_id);
    let mut ht = STORE_TRAFFIC_STATS_MAP.lock().unwrap();
    ht.insert(k, v);
}

pub(crate) fn push_upstream_traffic_stats(
    stats: Arc<UserUpstreamTrafficStats>,
    site_id: &MetricsName,
) {
    let k = stats.stat_id();
    let v = UpstreamTrafficStatsValue::new(stats, site_id);
    let mut ht = STORE_UPSTREAM_TRAFFIC_STATS_MAP.lock().unwrap();
    ht.insert(k, v);
}

pub(in crate::stat) fn sync_stats() {
    move_ht(&STORE_REQUEST_STATS_MAP, &USER_SITE_REQUEST_STATS_MAP);
    move_ht(&STORE_TRAFFIC_STATS_MAP, &USER_SITE_TRAFFIC_STATS_MAP);
    move_ht(
        &STORE_UPSTREAM_TRAFFIC_STATS_MAP,
        &USER_SITE_UPSTREAM_TRAFFIC_STATS_MAP,
    );
}

pub(in crate::stat) fn emit_stats(client: &StatsdClient) {
    let mut req_stats_map = USER_SITE_REQUEST_STATS_MAP.lock().unwrap();
    req_stats_map.retain(|_, v| {
        let names = RequestStatsNamesRef {
            connection_total: &v.names.connection_total,
            request_total: &v.names.request_total,
            request_alive: &v.names.request_alive,
            request_ready: &v.names.request_ready,
            request_reuse: &v.names.request_reuse,
            request_renew: &v.names.request_renew,
            l7_connection_alive: &v.names.l7_connection_alive,
        };
        super::user::emit_user_request_stats(client, &v.stats, &mut v.snap, &names);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(&v.stats) > 1
    });
    drop(req_stats_map);

    let mut io_stats_map = USER_SITE_TRAFFIC_STATS_MAP.lock().unwrap();
    io_stats_map.retain(|_, v| {
        let names = TrafficStatsNamesRef {
            in_bytes: &v.names.in_bytes,
            in_packets: &v.names.in_packets,
            out_bytes: &v.names.out_bytes,
            out_packets: &v.names.out_packets,
        };
        super::user::emit_user_traffic_stats(client, &v.stats, &mut v.snap, &names);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(&v.stats) > 1
    });
    drop(io_stats_map);

    let mut upstream_io_stats_map = USER_SITE_UPSTREAM_TRAFFIC_STATS_MAP.lock().unwrap();
    upstream_io_stats_map.retain(|_, v| {
        let names = TrafficStatsNamesRef {
            in_bytes: &v.tags.in_bytes,
            in_packets: &v.tags.in_packets,
            out_bytes: &v.tags.out_bytes,
            out_packets: &v.tags.out_packets,
        };
        super::user::emit_user_upstream_traffic_stats(client, &v.stats, &mut v.snap, &names);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(&v.stats) > 1
    });
    drop(upstream_io_stats_map);
}
