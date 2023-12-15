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

use std::sync::Arc;

use g3_statsd_client::{StatsdClient, StatsdTagGroup};

use super::ServerMetricExt;
use crate::listen::{ListenSnapshot, ListenStats};

const METRIC_NAME_LISTEN_INSTANCE_COUNT: &str = "listen.instance.count";
const METRIC_NAME_LISTEN_ACCEPTED: &str = "listen.accepted";
const METRIC_NAME_LISTEN_DROPPED: &str = "listen.dropped";
const METRIC_NAME_LISTEN_TIMEOUT: &str = "listen.timeout";
const METRIC_NAME_LISTEN_FAILED: &str = "listen.failed";

pub fn emit_listen_stats(
    client: &mut StatsdClient,
    stats: &Arc<ListenStats>,
    snap: &mut ListenSnapshot,
) {
    let mut common_tags = StatsdTagGroup::default();
    common_tags.add_server_tags(stats.name(), stats.is_running(), stats.stat_id());

    client
        .gauge_with_tags(
            METRIC_NAME_LISTEN_INSTANCE_COUNT,
            stats.get_running_runtime_count(),
            &common_tags,
        )
        .send();

    macro_rules! emit_field {
        ($field:ident, $name:expr) => {
            let new_value = stats.$field();
            if new_value != 0 || snap.$field != 0 {
                let diff_value = new_value.wrapping_sub(snap.$field);
                client
                    .count_with_tags($name, diff_value, &common_tags)
                    .send();
                snap.$field = new_value;
            }
        };
    }

    emit_field!(accepted, METRIC_NAME_LISTEN_ACCEPTED);
    emit_field!(dropped, METRIC_NAME_LISTEN_DROPPED);
    emit_field!(timeout, METRIC_NAME_LISTEN_TIMEOUT);
    emit_field!(failed, METRIC_NAME_LISTEN_FAILED);
}
