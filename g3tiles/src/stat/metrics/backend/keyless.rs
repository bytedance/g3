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

use std::sync::{Arc, LazyLock, Mutex};

use ahash::AHashMap;

use g3_daemon::metrics::TAG_KEY_QUANTILE;
use g3_statsd_client::{StatsdClient, StatsdTagGroup};
use g3_types::stats::StatId;

use super::BackendMetricExt;
use crate::module::keyless::{KeylessBackendStats, KeylessUpstreamDurationStats};

const METRIC_NAME_KEYLESS_CONN_ATTEMPT: &str = "backend.keyless.connection.attempt";
const METRIC_NAME_KEYLESS_CONN_ESTABLISHED: &str = "backend.keyless.connection.established";
const METRIC_NAME_KEYLESS_CHANNEL_ALIVE: &str = "backend.keyless.channel.alive";
const METRIC_NAME_KEYLESS_REQUEST_RECV: &str = "backend.keyless.request.recv";
const METRIC_NAME_KEYLESS_REQUEST_SEND: &str = "backend.keyless.request.send";
const METRIC_NAME_KEYLESS_REQUEST_DROP: &str = "backend.keyless.request.drop";
const METRIC_NAME_KEYLESS_RESPONSE_RECV: &str = "backend.keyless.response.recv";
const METRIC_NAME_KEYLESS_RESPONSE_SEND: &str = "backend.keyless.response.send";
const METRIC_NAME_KEYLESS_RESPONSE_DROP: &str = "backend.keyless.response.drop";

const METRIC_NAME_KEYLESS_CONNECT_DURATION: &str = "backend.keyless.connect.duration";
const METRIC_NAME_KEYLESS_WAIT_DURATION: &str = "backend.keyless.wait.duration";
const METRIC_NAME_KEYLESS_RESPONSE_DURATION: &str = "backend.keyless.response.duration";

type KeylessBackendStatsValue = (Arc<KeylessBackendStats>, KeylessBackendSnapshot);

static STORE_KEYLESS_STATS_MAP: LazyLock<Mutex<AHashMap<StatId, KeylessBackendStatsValue>>> =
    LazyLock::new(|| Mutex::new(AHashMap::new()));
static KEYLESS_STATS_MAP: LazyLock<Mutex<AHashMap<StatId, KeylessBackendStatsValue>>> =
    LazyLock::new(|| Mutex::new(AHashMap::new()));
static STORE_KEYLESS_DURATION_STATS_MAP: LazyLock<
    Mutex<AHashMap<StatId, Arc<KeylessUpstreamDurationStats>>>,
> = LazyLock::new(|| Mutex::new(AHashMap::new()));
static KEYLESS_DURATION_STATS_MAP: LazyLock<
    Mutex<AHashMap<StatId, Arc<KeylessUpstreamDurationStats>>>,
> = LazyLock::new(|| Mutex::new(AHashMap::new()));

#[derive(Default)]
struct KeylessBackendSnapshot {
    conn_attempt: u64,
    conn_established: u64,
    request_recv: u64,
    request_send: u64,
    request_drop: u64,
    response_recv: u64,
    response_send: u64,
    response_drop: u64,
}

pub(crate) fn push_keyless_stats(stats: Arc<KeylessBackendStats>) {
    let k = stats.stat_id();
    let mut ht = STORE_KEYLESS_STATS_MAP.lock().unwrap();
    ht.insert(k, (stats, KeylessBackendSnapshot::default()));
}

pub(crate) fn push_keyless_duration_stats(stats: Arc<KeylessUpstreamDurationStats>) {
    let k = stats.stat_id();
    let mut ht = STORE_KEYLESS_DURATION_STATS_MAP.lock().unwrap();
    ht.insert(k, stats);
}

pub(super) fn sync_stats() {
    use g3_daemon::metrics::helper::move_ht;

    move_ht(&STORE_KEYLESS_STATS_MAP, &KEYLESS_STATS_MAP);
    move_ht(
        &STORE_KEYLESS_DURATION_STATS_MAP,
        &KEYLESS_DURATION_STATS_MAP,
    );
}

pub(super) fn emit_stats(client: &mut StatsdClient) {
    let mut backend_stats_map = KEYLESS_STATS_MAP.lock().unwrap();
    backend_stats_map.retain(|_, (stats, snap)| {
        emit_keyless_stats(client, stats, snap);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
    drop(backend_stats_map);

    let mut duration_stats_map = KEYLESS_DURATION_STATS_MAP.lock().unwrap();
    duration_stats_map.retain(|_, stats| {
        emit_keyless_duration_stats(client, stats);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
    drop(duration_stats_map);
}

fn emit_keyless_stats(
    client: &mut StatsdClient,
    stats: &Arc<KeylessBackendStats>,
    snap: &mut KeylessBackendSnapshot,
) {
    let mut common_tags = StatsdTagGroup::default();
    common_tags.add_backend_tags(stats.name(), stats.stat_id());
    if let Some(tags) = stats.load_extra_tags() {
        common_tags.add_static_tags(&tags);
    }

    let channel_alive = stats.alive_channel();
    client
        .gauge_with_tags(
            METRIC_NAME_KEYLESS_CHANNEL_ALIVE,
            channel_alive,
            &common_tags,
        )
        .send();

    macro_rules! emit_count {
        ($field:ident, $name:expr) => {
            let new_value = stats.$field();
            let diff_value = new_value.wrapping_sub(snap.$field);
            client
                .count_with_tags($name, diff_value, &common_tags)
                .send();
            snap.$field = new_value;
        };
    }

    emit_count!(conn_attempt, METRIC_NAME_KEYLESS_CONN_ATTEMPT);
    emit_count!(conn_established, METRIC_NAME_KEYLESS_CONN_ESTABLISHED);

    emit_count!(request_recv, METRIC_NAME_KEYLESS_REQUEST_RECV);
    emit_count!(request_send, METRIC_NAME_KEYLESS_REQUEST_SEND);
    emit_count!(request_drop, METRIC_NAME_KEYLESS_REQUEST_DROP);

    emit_count!(response_recv, METRIC_NAME_KEYLESS_RESPONSE_RECV);
    emit_count!(response_send, METRIC_NAME_KEYLESS_RESPONSE_SEND);
    emit_count!(response_drop, METRIC_NAME_KEYLESS_RESPONSE_DROP);
}

fn emit_keyless_duration_stats(
    client: &mut StatsdClient,
    stats: &Arc<KeylessUpstreamDurationStats>,
) {
    let mut common_tags = StatsdTagGroup::default();
    common_tags.add_backend_tags(stats.name(), stats.stat_id());
    if let Some(tags) = stats.load_extra_tags() {
        common_tags.add_static_tags(&tags);
    }

    macro_rules! emit_duration {
        ($field:ident, $name:expr) => {
            stats.$field.foreach_stat(|_, qs, v| {
                if v > 0_f64 {
                    client
                        .gauge_float_with_tags($name, v, &common_tags)
                        .with_tag(TAG_KEY_QUANTILE, qs)
                        .send();
                }
            })
        };
    }

    emit_duration!(connect, METRIC_NAME_KEYLESS_CONNECT_DURATION);
    emit_duration!(wait, METRIC_NAME_KEYLESS_WAIT_DURATION);
    emit_duration!(response, METRIC_NAME_KEYLESS_RESPONSE_DURATION);
}
