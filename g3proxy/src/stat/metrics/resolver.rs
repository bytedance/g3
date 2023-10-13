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
use cadence::{Counted, Gauged, Metric, MetricBuilder, StatsdClient};
use once_cell::sync::Lazy;

use g3_daemon::metric::TAG_KEY_STAT_ID;
use g3_resolver::{
    ResolveQueryType, ResolverMemorySnapshot, ResolverQuerySnapshot, ResolverSnapshot,
};
use g3_types::metrics::MetricsName;
use g3_types::stats::StatId;

use crate::resolve::ResolverStats;

const TAG_KEY_RESOLVER: &str = "resolver";
const TAG_KEY_RR_TYPE: &str = "rr_type";

const METRIC_NAME_QUERY_TOTAL: &str = "resolver.query.total";
const METRIC_NAME_QUERY_CACHED: &str = "resolver.query.cached";
const METRIC_NAME_QUERY_DRIVER: &str = "resolver.query.driver.total";
const METRIC_NAME_QUERY_DRIVER_TIMEOUT: &str = "resolver.query.driver.timeout";
const METRIC_NAME_QUERY_DRIVER_REFUSED: &str = "resolver.query.driver.refused";
const METRIC_NAME_QUERY_DRIVER_MALFORMED: &str = "resolver.query.driver.malformed";
const METRIC_NAME_QUERY_SERVER_REFUSED: &str = "resolver.query.server.refused";
const METRIC_NAME_QUERY_SERVER_MALFORMED: &str = "resolver.query.server.malformed";
const METRIC_NAME_QUERY_SERVER_NOT_FOUND: &str = "resolver.query.server.not_found";
const METRIC_NAME_QUERY_SERVER_SERV_FAIL: &str = "resolver.query.server.serv_fail";
const METRIC_NAME_MEMORY_CACHE_CAPACITY: &str = "resolver.memory.cache.capacity";
const METRIC_NAME_MEMORY_CACHE_LENGTH: &str = "resolver.memory.cache.length";
const METRIC_NAME_MEMORY_DOING_CAPACITY: &str = "resolver.memory.doing.capacity";
const METRIC_NAME_MEMORY_DOING_LENGTH: &str = "resolver.memory.doing.length";

type ResolverStatsValue = (Arc<ResolverStats>, ResolverSnapshot);

static RESOLVER_STATS_MAP: Lazy<Mutex<AHashMap<StatId, ResolverStatsValue>>> =
    Lazy::new(|| Mutex::new(AHashMap::new()));

trait ResolverMetricExt<'m> {
    fn add_resolver_tags(
        self,
        resolver: &'m MetricsName,
        rr_type: &'m str,
        stat_id: &'m str,
    ) -> Self;
}

impl<'m, 'c, T> ResolverMetricExt<'m> for MetricBuilder<'m, 'c, T>
where
    T: Metric + From<String>,
{
    fn add_resolver_tags(
        self,
        resolver: &'m MetricsName,
        rr_type: &'m str,
        stat_id: &'m str,
    ) -> Self {
        self.with_tag(TAG_KEY_RESOLVER, resolver.as_str())
            .with_tag(TAG_KEY_RR_TYPE, rr_type)
            .with_tag(TAG_KEY_STAT_ID, stat_id)
    }
}

pub(in crate::stat) fn sync_stats() {
    let mut stats_map = RESOLVER_STATS_MAP.lock().unwrap();
    crate::resolve::foreach_resolver(|_, server| {
        let stats = server.get_stats();
        let stat_id = stats.stat_id();
        stats_map
            .entry(stat_id)
            .or_insert_with(|| (stats, ResolverSnapshot::default()));
    });
}

pub(in crate::stat) fn emit_stats(client: &StatsdClient) {
    let mut stats_map = RESOLVER_STATS_MAP.lock().unwrap();
    stats_map.retain(|_, (stats, snap)| {
        emit_to_statsd(client, stats, snap);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
}

fn emit_to_statsd(client: &StatsdClient, stats: &ResolverStats, snap: &mut ResolverSnapshot) {
    let resolver = stats.name();
    let mut buffer = itoa::Buffer::new();
    let stat_id = buffer.format(stats.stat_id().as_u64());

    let inner_stats = stats.inner().snapshot();

    emit_query_stats_to_statsd(
        client,
        &inner_stats.query_a,
        &mut snap.query_a,
        resolver,
        ResolveQueryType::A,
        stat_id,
    );

    emit_query_stats_to_statsd(
        client,
        &inner_stats.query_aaaa,
        &mut snap.query_aaaa,
        resolver,
        ResolveQueryType::Aaaa,
        stat_id,
    );

    emit_memory_stats_to_statsd(
        client,
        &inner_stats.memory_a,
        resolver,
        ResolveQueryType::A,
        stat_id,
    );

    emit_memory_stats_to_statsd(
        client,
        &inner_stats.memory_aaaa,
        resolver,
        ResolveQueryType::Aaaa,
        stat_id,
    );
}

fn emit_query_stats_to_statsd(
    client: &StatsdClient,
    stats: &ResolverQuerySnapshot,
    snap: &mut ResolverQuerySnapshot,
    resolver: &MetricsName,
    rr_type: ResolveQueryType,
    stat_id: &str,
) {
    let rr_type = rr_type.as_str();

    let new_value = stats.total;
    if new_value == 0 && snap.total == 0 {
        return;
    }
    let diff_value = i64::try_from(new_value.wrapping_sub(snap.total)).unwrap_or(i64::MAX);
    client
        .count_with_tags(METRIC_NAME_QUERY_TOTAL, diff_value)
        .add_resolver_tags(resolver, rr_type, stat_id)
        .send();
    snap.total = new_value;

    macro_rules! emit_query_stats_u64 {
        ($id:ident, $name:expr) => {
            let new_value = stats.$id;
            if new_value != 0 || snap.$id != 0 {
                let diff_value =
                    i64::try_from(new_value.wrapping_sub(snap.$id)).unwrap_or(i64::MAX);
                client
                    .count_with_tags($name, diff_value)
                    .add_resolver_tags(resolver, rr_type, stat_id)
                    .send();
                snap.$id = new_value;
            }
        };
    }

    emit_query_stats_u64!(cached, METRIC_NAME_QUERY_CACHED);
    emit_query_stats_u64!(driver, METRIC_NAME_QUERY_DRIVER);
    emit_query_stats_u64!(driver_timeout, METRIC_NAME_QUERY_DRIVER_TIMEOUT);
    emit_query_stats_u64!(driver_refused, METRIC_NAME_QUERY_DRIVER_REFUSED);
    emit_query_stats_u64!(driver_malformed, METRIC_NAME_QUERY_DRIVER_MALFORMED);
    emit_query_stats_u64!(server_refused, METRIC_NAME_QUERY_SERVER_REFUSED);
    emit_query_stats_u64!(server_malformed, METRIC_NAME_QUERY_SERVER_MALFORMED);
    emit_query_stats_u64!(server_not_found, METRIC_NAME_QUERY_SERVER_NOT_FOUND);
    emit_query_stats_u64!(server_serv_fail, METRIC_NAME_QUERY_SERVER_SERV_FAIL);
}

fn emit_memory_stats_to_statsd(
    client: &StatsdClient,
    stats: &ResolverMemorySnapshot,
    resolver: &MetricsName,
    rr_type: ResolveQueryType,
    stat_id: &str,
) {
    let rr_type = rr_type.as_str();

    client
        .gauge_with_tags(METRIC_NAME_MEMORY_CACHE_CAPACITY, stats.cap_cache as u64)
        .add_resolver_tags(resolver, rr_type, stat_id)
        .send();
    client
        .gauge_with_tags(METRIC_NAME_MEMORY_CACHE_LENGTH, stats.len_cache as u64)
        .add_resolver_tags(resolver, rr_type, stat_id)
        .send();
    client
        .gauge_with_tags(METRIC_NAME_MEMORY_DOING_CAPACITY, stats.cap_doing as u64)
        .add_resolver_tags(resolver, rr_type, stat_id)
        .send();
    client
        .gauge_with_tags(METRIC_NAME_MEMORY_DOING_LENGTH, stats.len_doing as u64)
        .add_resolver_tags(resolver, rr_type, stat_id)
        .send();
}
