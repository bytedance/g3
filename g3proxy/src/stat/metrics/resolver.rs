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

use g3_daemon::metrics::TAG_KEY_STAT_ID;
use g3_resolver::{
    ResolveQueryType, ResolverMemorySnapshot, ResolverQuerySnapshot, ResolverSnapshot,
};
use g3_statsd_client::{StatsdClient, StatsdTagGroup};
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

trait ResolverMetricExt {
    fn add_resolver_tags(&mut self, resolver: &MetricsName, stat_id: StatId);
}

impl ResolverMetricExt for StatsdTagGroup {
    fn add_resolver_tags(&mut self, resolver: &MetricsName, stat_id: StatId) {
        let mut buffer = itoa::Buffer::new();
        let stat_id = buffer.format(stat_id.as_u64());
        self.add_tag(TAG_KEY_RESOLVER, resolver);
        self.add_tag(TAG_KEY_STAT_ID, stat_id);
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

pub(in crate::stat) fn emit_stats(client: &mut StatsdClient) {
    let mut stats_map = RESOLVER_STATS_MAP.lock().unwrap();
    stats_map.retain(|_, (stats, snap)| {
        emit_to_statsd(client, stats, snap);
        // use Arc instead of Weak here, as we should emit the final metrics before drop it
        Arc::strong_count(stats) > 1
    });
}

fn emit_to_statsd(client: &mut StatsdClient, stats: &ResolverStats, snap: &mut ResolverSnapshot) {
    let mut common_tags = StatsdTagGroup::default();
    common_tags.add_resolver_tags(stats.name(), stats.stat_id());

    let inner_stats = stats.inner().snapshot();

    emit_query_stats_to_statsd(
        client,
        &inner_stats.query_a,
        &mut snap.query_a,
        &common_tags,
        ResolveQueryType::A,
    );

    emit_query_stats_to_statsd(
        client,
        &inner_stats.query_aaaa,
        &mut snap.query_aaaa,
        &common_tags,
        ResolveQueryType::Aaaa,
    );

    emit_memory_stats_to_statsd(
        client,
        &inner_stats.memory_a,
        &common_tags,
        ResolveQueryType::A,
    );

    emit_memory_stats_to_statsd(
        client,
        &inner_stats.memory_aaaa,
        &common_tags,
        ResolveQueryType::Aaaa,
    );
}

fn emit_query_stats_to_statsd(
    client: &mut StatsdClient,
    stats: &ResolverQuerySnapshot,
    snap: &mut ResolverQuerySnapshot,
    common_tags: &StatsdTagGroup,
    rr_type: ResolveQueryType,
) {
    if stats.total == 0 && snap.total == 0 {
        return;
    }

    let rr_type = rr_type.as_str();

    let new_value = stats.total;
    let diff_value = new_value.wrapping_sub(snap.total);
    client
        .count_with_tags(METRIC_NAME_QUERY_TOTAL, diff_value, common_tags)
        .with_tag(TAG_KEY_RR_TYPE, rr_type)
        .send();
    snap.total = new_value;

    macro_rules! emit_query_stats_u64 {
        ($id:ident, $name:expr) => {
            let new_value = stats.$id;
            if new_value != 0 || snap.$id != 0 {
                let diff_value = new_value.wrapping_sub(snap.$id);
                client
                    .count_with_tags($name, diff_value, common_tags)
                    .with_tag(TAG_KEY_RR_TYPE, rr_type)
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
    client: &mut StatsdClient,
    snap: &ResolverMemorySnapshot,
    common_tags: &StatsdTagGroup,
    rr_type: ResolveQueryType,
) {
    macro_rules! emit_field {
        ($field:ident, $name:expr) => {
            client
                .gauge_with_tags($name, snap.$field, common_tags)
                .with_tag(TAG_KEY_RR_TYPE, rr_type)
                .send();
        };
    }

    emit_field!(cap_cache, METRIC_NAME_MEMORY_CACHE_CAPACITY);
    emit_field!(len_cache, METRIC_NAME_MEMORY_CACHE_LENGTH);
    emit_field!(cap_doing, METRIC_NAME_MEMORY_DOING_CAPACITY);
    emit_field!(len_doing, METRIC_NAME_MEMORY_DOING_LENGTH);
}
