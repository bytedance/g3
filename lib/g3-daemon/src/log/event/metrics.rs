/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use foldhash::fast::FixedState;

use g3_statsd_client::{StatsdClient, StatsdTagGroup};
use g3_types::log::LogSnapshot;
use g3_types::stats::StatId;

use super::LoggerStats;
use crate::metrics::LoggerMetricExt;

type LoggerStatsValue = (Arc<LoggerStats>, LogSnapshot);

static LOGGER_STATS_MAP: Mutex<HashMap<StatId, LoggerStatsValue, FixedState>> =
    Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));

pub fn sync_stats() {
    let mut stats_map = LOGGER_STATS_MAP.lock().unwrap();
    super::registry::foreach_stats(|_, stats| {
        let stat_id = stats.stat_id();
        stats_map
            .entry(stat_id)
            .or_insert_with(|| (Arc::clone(stats), LogSnapshot::default()));
    });
}

pub fn emit_stats(client: &mut StatsdClient) {
    let mut stats_map = LOGGER_STATS_MAP.lock().unwrap();
    stats_map.retain(|_, (stats, snap)| {
        emit_to_statsd(client, stats, snap);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1 || Arc::strong_count(stats.inner()) > 1
    });
}

fn emit_to_statsd(client: &mut StatsdClient, stats: &LoggerStats, snap: &mut LogSnapshot) {
    let log_stats = stats.inner().snapshot();

    let mut common_tags = StatsdTagGroup::default();
    common_tags.add_logger_tags(stats.name(), stats.stat_id());

    crate::metrics::emit_log_io_stats(client, &log_stats.io, &mut snap.io, &common_tags);
    crate::metrics::emit_log_drop_stats(client, &log_stats.drop, &mut snap.drop, &common_tags);
}
