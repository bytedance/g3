/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::{Arc, Mutex};

use g3_daemon::metrics::TAG_KEY_QUANTILE;
use g3_histogram::HistogramStats;
use g3_statsd_client::{StatsdClient, StatsdTagGroup};
use g3_types::stats::GlobalStatsMap;

use super::BackendMetricExt;
use crate::module::stream::{StreamBackendDurationStats, StreamBackendStats};

const METRIC_NAME_STREAM_CONN_ATTEMPT: &str = "backend.stream.connection.attempt";
const METRIC_NAME_STREAM_CONN_ESTABLISHED: &str = "backend.stream.connection.established";

const METRIC_NAME_STREAM_CONNECT_DURATION: &str = "backend.stream.connect.duration";

type StreamBackendStatsValue = (Arc<StreamBackendStats>, StreamBackendSnapshot);

static STORE_STREAM_STATS_MAP: Mutex<GlobalStatsMap<StreamBackendStatsValue>> =
    Mutex::new(GlobalStatsMap::new());
static STREAM_STATS_MAP: Mutex<GlobalStatsMap<StreamBackendStatsValue>> =
    Mutex::new(GlobalStatsMap::new());
static STORE_STREAM_DURATION_STATS_MAP: Mutex<GlobalStatsMap<Arc<StreamBackendDurationStats>>> =
    Mutex::new(GlobalStatsMap::new());
static STREAM_DURATION_STATS_MAP: Mutex<GlobalStatsMap<Arc<StreamBackendDurationStats>>> =
    Mutex::new(GlobalStatsMap::new());

#[derive(Default)]
struct StreamBackendSnapshot {
    conn_attempt: u64,
    conn_established: u64,
}

pub(crate) fn push_stream_stats(stats: Arc<StreamBackendStats>) {
    let k = stats.stat_id();
    let mut ht = STORE_STREAM_STATS_MAP.lock().unwrap();
    ht.insert(k, (stats, StreamBackendSnapshot::default()));
}

pub(crate) fn push_stream_duration_stats(stats: Arc<StreamBackendDurationStats>) {
    let k = stats.stat_id();
    let mut ht = STORE_STREAM_DURATION_STATS_MAP.lock().unwrap();
    ht.insert(k, stats);
}

pub(super) fn sync_stats() {
    use g3_daemon::metrics::helper::move_ht;

    move_ht(&STORE_STREAM_STATS_MAP, &STREAM_STATS_MAP);
    move_ht(&STORE_STREAM_DURATION_STATS_MAP, &STREAM_DURATION_STATS_MAP);
}

pub(super) fn emit_stats(client: &mut StatsdClient) {
    let mut backend_stats_map = STREAM_STATS_MAP.lock().unwrap();
    backend_stats_map.retain(|(stats, snap)| {
        emit_stream_stats(client, stats, snap);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
    drop(backend_stats_map);

    let mut duration_stats_map = STREAM_DURATION_STATS_MAP.lock().unwrap();
    duration_stats_map.retain(|stats| {
        emit_stream_duration_stats(client, stats);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
    drop(duration_stats_map);
}

fn emit_stream_stats(
    client: &mut StatsdClient,
    stats: &Arc<StreamBackendStats>,
    snap: &mut StreamBackendSnapshot,
) {
    let mut common_tags = StatsdTagGroup::default();
    common_tags.add_backend_tags(stats.name(), stats.stat_id());
    if let Some(tags) = stats.load_extra_tags() {
        common_tags.add_static_tags(&tags);
    }

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

    emit_count!(conn_attempt, METRIC_NAME_STREAM_CONN_ATTEMPT);
    emit_count!(conn_established, METRIC_NAME_STREAM_CONN_ESTABLISHED);
}

fn emit_stream_duration_stats(client: &mut StatsdClient, stats: &Arc<StreamBackendDurationStats>) {
    let mut common_tags = StatsdTagGroup::default();
    common_tags.add_backend_tags(stats.name(), stats.stat_id());
    if let Some(tags) = stats.load_extra_tags() {
        common_tags.add_static_tags(&tags);
    }

    emit_stream_connect_duration_stats(client, &stats.connect, &common_tags);
}

fn emit_stream_connect_duration_stats(
    client: &mut StatsdClient,
    stats: &HistogramStats,
    common_tags: &StatsdTagGroup,
) {
    stats.foreach_stat(|_, qs, v| {
        if v > 0_f64 {
            client
                .gauge_float_with_tags(METRIC_NAME_STREAM_CONNECT_DURATION, v, common_tags)
                .with_tag(TAG_KEY_QUANTILE, qs)
                .send();
        }
    })
}
