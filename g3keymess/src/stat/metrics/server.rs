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

use g3_daemon::listen::{ListenSnapshot, ListenStats};
use g3_daemon::metrics::ServerMetricExt;
use g3_histogram::HistogramStats;
use g3_statsd_client::{StatsdClient, StatsdTagGroup};
use g3_types::stats::StatId;

use crate::serve::{
    KeyServerDurationStats, KeyServerRequestSnapshot, KeyServerSnapshot, KeyServerStats,
};

const TAG_KEY_REQUEST: &str = "request";
const TAG_KEY_REASON: &str = "reason";
const TAG_KEY_QUANTILE: &str = "quantile";

const METRIC_NAME_SERVER_TASK_TOTAL: &str = "server.task.total";
const METRIC_NAME_SERVER_TASK_ALIVE: &str = "server.task.alive";

const METRIC_NAME_SERVER_REQUEST_TOTAL: &str = "server.request.total";
const METRIC_NAME_SERVER_REQUEST_ALIVE: &str = "server.request.alive";
const METRIC_NAME_SERVER_REQUEST_PASSED: &str = "server.request.passed";
const METRIC_NAME_SERVER_REQUEST_FAILED: &str = "server.request.failed";
const METRIC_NAME_SERVER_REQUEST_DURATION: &str = "server.request.duration";

const REQUEST_TYPE_NO_OP: &str = "no_op";
const REQUEST_TYPE_PING_PONG: &str = "ping_pong";
const REQUEST_TYPE_RSA_DECRYPT: &str = "rsa_decrypt";
const REQUEST_TYPE_RSA_SIGN: &str = "rsa_sign";
const REQUEST_TYPE_RSA_PSS_SIGN: &str = "rsa_pss_sign";
const REQUEST_TYPE_ECDSA_SIGN: &str = "ecdsa_sign";
const REQUEST_TYPE_ED25519_SIGN: &str = "ed25519_sign";

const FAIL_REASON_KEY_NOT_FOUND: &str = "key_not_found";
const FAIL_REASON_CRYPTO_FAIL: &str = "crypto_fail";
const FAIL_REASON_BAD_OP_CODE: &str = "bad_op_code";
const FAIL_REASON_FORMAT_ERROR: &str = "format_error";
const FAIL_REASON_OTHER_FAIL: &str = "other_fail";

type ServerStatsValue = (Arc<KeyServerStats>, KeyServerSnapshot);
type ListenStatsValue = (Arc<ListenStats>, ListenSnapshot);

static SERVER_STATS_MAP: Lazy<Mutex<AHashMap<StatId, ServerStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));
static LISTEN_STATS_MAP: Lazy<Mutex<AHashMap<StatId, ListenStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));
static DURATION_STATS_MAP: Lazy<Mutex<AHashMap<StatId, Arc<KeyServerDurationStats>>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));

pub(in crate::stat) fn sync_stats() {
    let mut server_stats_map = SERVER_STATS_MAP.lock().unwrap();
    crate::serve::foreach_server(|_, server| {
        let stats = server.get_server_stats();
        let stat_id = stats.stat_id();
        server_stats_map
            .entry(stat_id)
            .or_insert_with(|| (stats, KeyServerSnapshot::default()));
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

    let mut duration_stats_map = DURATION_STATS_MAP.lock().unwrap();
    crate::serve::foreach_server(|_, server| {
        let stats = server.get_duration_stats();
        let stat_id = stats.stat_id();
        duration_stats_map.entry(stat_id).or_insert_with(|| stats);
    });
    drop(duration_stats_map);
}

pub(in crate::stat) fn emit_stats(client: &mut StatsdClient) {
    let mut server_stats_map = SERVER_STATS_MAP.lock().unwrap();
    server_stats_map.retain(|_, (stats, snap)| {
        emit_server_stats(client, stats, snap);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
    drop(server_stats_map);

    let mut listen_stats_map = LISTEN_STATS_MAP.lock().unwrap();
    listen_stats_map.retain(|_, (stats, snap)| {
        g3_daemon::metrics::emit_listen_stats(client, stats, snap);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
    drop(listen_stats_map);

    let mut duration_stats_map = DURATION_STATS_MAP.lock().unwrap();
    duration_stats_map.retain(|_, stats| {
        emit_server_duration_stats(client, stats);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
    drop(duration_stats_map);
}

fn emit_server_stats(
    client: &mut StatsdClient,
    stats: &Arc<KeyServerStats>,
    snap: &mut KeyServerSnapshot,
) {
    let mut common_tags = StatsdTagGroup::default();
    common_tags.add_server_tags(stats.name(), stats.is_online(), stats.stat_id());
    if let Some(tags) = stats.load_extra_tags() {
        common_tags.add_static_tags(&tags);
    }

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

    macro_rules! emit_request_stats_u64 {
        ($id:ident, $request:expr) => {
            emit_server_request_stats(
                client,
                $request,
                stats.$id.snapshot(),
                &mut snap.$id,
                &common_tags,
            );
        };
    }
    emit_request_stats_u64!(noop, REQUEST_TYPE_NO_OP);
    emit_request_stats_u64!(ping_pong, REQUEST_TYPE_PING_PONG);
    emit_request_stats_u64!(rsa_decrypt, REQUEST_TYPE_RSA_DECRYPT);
    emit_request_stats_u64!(rsa_sign, REQUEST_TYPE_RSA_SIGN);
    emit_request_stats_u64!(rsa_pss_sign, REQUEST_TYPE_RSA_PSS_SIGN);
    emit_request_stats_u64!(ecdsa_sign, REQUEST_TYPE_ECDSA_SIGN);
    emit_request_stats_u64!(ed25519_sign, REQUEST_TYPE_ED25519_SIGN);
}

fn emit_server_request_stats(
    client: &mut StatsdClient,
    request: &str,
    stats: KeyServerRequestSnapshot,
    snap: &mut KeyServerRequestSnapshot,
    common_tags: &StatsdTagGroup,
) {
    let new_value = stats.total;
    if new_value == 0 && snap.total == 0 {
        return;
    }
    let diff_value = new_value.wrapping_sub(snap.total);
    client
        .count_with_tags(METRIC_NAME_SERVER_REQUEST_TOTAL, diff_value, common_tags)
        .with_tag(TAG_KEY_REQUEST, request)
        .send();
    snap.total = new_value;

    client
        .gauge_with_tags(
            METRIC_NAME_SERVER_REQUEST_ALIVE,
            stats.alive_count,
            common_tags,
        )
        .with_tag(TAG_KEY_REQUEST, request)
        .send();

    let new_value = stats.passed;
    let diff_value = new_value.wrapping_sub(snap.passed);
    client
        .count_with_tags(METRIC_NAME_SERVER_REQUEST_PASSED, diff_value, common_tags)
        .with_tag(TAG_KEY_REQUEST, request)
        .send();
    snap.passed = new_value;

    macro_rules! emit_failed_stats_u64 {
        ($id:ident, $reason:expr) => {
            let new_value = stats.$id;
            if new_value != 0 || snap.$id != 0 {
                let diff_value = new_value.wrapping_sub(snap.$id);
                client
                    .count_with_tags(METRIC_NAME_SERVER_REQUEST_FAILED, diff_value, common_tags)
                    .with_tag(TAG_KEY_REQUEST, request)
                    .with_tag(TAG_KEY_REASON, $reason)
                    .send();
                snap.$id = new_value;
            }
        };
    }
    emit_failed_stats_u64!(key_not_found, FAIL_REASON_KEY_NOT_FOUND);
    emit_failed_stats_u64!(crypto_fail, FAIL_REASON_CRYPTO_FAIL);
    emit_failed_stats_u64!(bad_op_code, FAIL_REASON_BAD_OP_CODE);
    emit_failed_stats_u64!(format_error, FAIL_REASON_FORMAT_ERROR);
    emit_failed_stats_u64!(other_fail, FAIL_REASON_OTHER_FAIL);
}

fn emit_server_duration_stats(client: &mut StatsdClient, stats: &Arc<KeyServerDurationStats>) {
    let mut common_tags = StatsdTagGroup::default();
    common_tags.add_server_tags(stats.name(), stats.is_online(), stats.stat_id());
    if let Some(tags) = stats.load_extra_tags() {
        common_tags.add_static_tags(&tags);
    }

    macro_rules! emit_request_stats_u64 {
        ($id:ident, $request:expr) => {
            emit_server_request_duration_stats(client, $request, &stats.$id, &common_tags);
        };
    }
    emit_request_stats_u64!(ping_pong, REQUEST_TYPE_PING_PONG);
    emit_request_stats_u64!(rsa_decrypt, REQUEST_TYPE_RSA_DECRYPT);
    emit_request_stats_u64!(rsa_sign, REQUEST_TYPE_RSA_SIGN);
    emit_request_stats_u64!(rsa_pss_sign, REQUEST_TYPE_RSA_PSS_SIGN);
    emit_request_stats_u64!(ecdsa_sign, REQUEST_TYPE_ECDSA_SIGN);
    emit_request_stats_u64!(ed25519_sign, REQUEST_TYPE_ED25519_SIGN);
}

fn emit_server_request_duration_stats(
    client: &mut StatsdClient,
    request: &str,
    stats: &HistogramStats,
    common_tags: &StatsdTagGroup,
) {
    stats.foreach_stat(|_, qs, v| {
        if v > 0_f64 {
            client
                .gauge_float_with_tags(METRIC_NAME_SERVER_REQUEST_DURATION, v, common_tags)
                .with_tag(TAG_KEY_REQUEST, request)
                .with_tag(TAG_KEY_QUANTILE, qs)
                .send();
        }
    })
}
