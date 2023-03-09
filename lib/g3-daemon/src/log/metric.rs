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

use std::convert::TryFrom;
use std::sync::{Arc, Mutex};

use ahash::AHashMap;
use cadence::{Counted, Metric, MetricBuilder, StatsdClient};
use once_cell::sync::Lazy;

use g3_types::log::{LogDropSnapshot, LogDropType, LogIoSnapshot, LogSnapshot};
use g3_types::stats::StatId;

use super::LoggerStats;
use crate::metric::TAG_KEY_STAT_ID;

const TAG_KEY_LOGGER: &str = "logger";
const TAG_KEY_DROP_TYPE: &str = "drop_type";

const METRIC_NAME_MESSAGE_TOTAL: &str = "logger.message.total";
const METRIC_NAME_MESSAGE_PASS: &str = "logger.message.pass";
const METRIC_NAME_TRAFFIC_PASS: &str = "logger.traffic.pass";
const METRIC_NAME_MESSAGE_DROP: &str = "logger.message.drop";

type LoggerStatsValue = (Arc<LoggerStats>, LogSnapshot);

static LOGGER_STATS_MAP: Lazy<Mutex<AHashMap<StatId, LoggerStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));

trait LoggerMetricExt<'m> {
    fn add_logger_tags(self, logger: &'m str, stat_id: &'m str) -> Self;
    fn add_drop_tags(self, logger: &'m str, stat_id: &'m str, drop_type: LogDropType) -> Self;
}

impl<'m, 'c, T> LoggerMetricExt<'m> for MetricBuilder<'m, 'c, T>
where
    T: Metric + From<String>,
{
    fn add_logger_tags(self, logger: &'m str, stat_id: &'m str) -> Self {
        self.with_tag(TAG_KEY_LOGGER, logger)
            .with_tag(TAG_KEY_STAT_ID, stat_id)
    }

    fn add_drop_tags(self, logger: &'m str, stat_id: &'m str, drop_type: LogDropType) -> Self {
        self.add_logger_tags(logger, stat_id)
            .with_tag(TAG_KEY_DROP_TYPE, drop_type.as_str())
    }
}

pub fn sync_stats() {
    let mut stats_map = LOGGER_STATS_MAP.lock().unwrap();
    super::registry::foreach_stats(|_, stats| {
        let stat_id = stats.stat_id();
        stats_map
            .entry(stat_id)
            .or_insert_with(|| (Arc::clone(stats), LogSnapshot::default()));
    });
}

pub fn emit_stats(client: &StatsdClient) {
    let mut stats_map = LOGGER_STATS_MAP.lock().unwrap();
    stats_map.retain(|_, (stats, snap)| {
        emit_to_statsd(client, stats, snap);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1 || Arc::strong_count(stats.inner()) > 1
    });
}

fn emit_to_statsd(client: &StatsdClient, stats: &LoggerStats, snap: &mut LogSnapshot) {
    let logger = stats.name();
    let mut buffer = itoa::Buffer::new();
    let stat_id = buffer.format(stats.stat_id().as_u64());

    let log_stats = stats.inner().snapshot();

    emit_io_stats_to_statsd(client, &log_stats.io, &mut snap.io, logger, stat_id);

    emit_drop_stats_to_statsd(client, &log_stats.drop, &mut snap.drop, logger, stat_id);
}

fn emit_io_stats_to_statsd(
    client: &StatsdClient,
    stats: &LogIoSnapshot,
    snap: &mut LogIoSnapshot,
    logger: &str,
    stat_id: &str,
) {
    let new_value = stats.total;
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.total)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_MESSAGE_TOTAL, diff_value)
        .add_logger_tags(logger, stat_id)
        .send();
    snap.total = new_value;

    let new_value = stats.passed;
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.passed)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_MESSAGE_PASS, diff_value)
        .add_logger_tags(logger, stat_id)
        .send();
    snap.passed = new_value;

    let new_value = stats.size;
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.size)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_TRAFFIC_PASS, diff_value)
        .add_logger_tags(logger, stat_id)
        .send();
    snap.size = new_value;
}

fn emit_drop_stats_to_statsd(
    client: &StatsdClient,
    stats: &LogDropSnapshot,
    snap: &mut LogDropSnapshot,
    logger: &str,
    stat_id: &str,
) {
    let new_value = stats.format_failed;
    if new_value != 0 || snap.format_failed != 0 {
        let diff_value =
            i64::try_from(new_value.wrapping_sub(snap.format_failed)).unwrap_or(i64::MAX);
        client
            .count_with_tags(METRIC_NAME_MESSAGE_DROP, diff_value)
            .add_drop_tags(logger, stat_id, LogDropType::FormatFailed)
            .send();
        snap.format_failed = new_value;
    }

    let new_value = stats.channel_closed;
    if new_value != 0 || snap.channel_closed != 0 {
        let diff_value =
            i64::try_from(new_value.wrapping_sub(snap.channel_closed)).unwrap_or(i64::MAX);
        client
            .count_with_tags(METRIC_NAME_MESSAGE_DROP, diff_value)
            .add_drop_tags(logger, stat_id, LogDropType::ChannelClosed)
            .send();
        snap.channel_closed = new_value;
    }

    let new_value = stats.channel_overflow;
    if new_value != 0 || snap.channel_overflow != 0 {
        let diff_value =
            i64::try_from(new_value.wrapping_sub(snap.channel_overflow)).unwrap_or(i64::MAX);
        client
            .count_with_tags(METRIC_NAME_MESSAGE_DROP, diff_value)
            .add_drop_tags(logger, stat_id, LogDropType::ChannelOverflow)
            .send();
        snap.channel_overflow = new_value;
    }

    let new_value = stats.peer_unreachable;
    if new_value != 0 || snap.peer_unreachable != 0 {
        let diff_value =
            i64::try_from(new_value.wrapping_sub(snap.peer_unreachable)).unwrap_or(i64::MAX);
        client
            .count_with_tags(METRIC_NAME_MESSAGE_DROP, diff_value)
            .add_drop_tags(logger, stat_id, LogDropType::PeerUnreachable)
            .send();
        snap.peer_unreachable = new_value;
    }
}
