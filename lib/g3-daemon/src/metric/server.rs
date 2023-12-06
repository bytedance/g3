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

use g3_statsd_client::StatsdTagGroup;
use g3_types::metrics::MetricsName;
use g3_types::stats::StatId;

use super::TAG_KEY_STAT_ID;

pub const TAG_KEY_SERVER: &str = "server";
pub const TAG_KEY_ONLINE: &str = "online";

pub trait ServerMetricExt {
    fn add_server_tags(&mut self, server: &MetricsName, online: bool, stat_id: StatId);
}

impl ServerMetricExt for StatsdTagGroup {
    fn add_server_tags(&mut self, server: &MetricsName, online: bool, stat_id: StatId) {
        let mut buffer = itoa::Buffer::new();
        let stat_id = buffer.format(stat_id.as_u64());

        self.add_tag(TAG_KEY_SERVER, server);

        let online_value = if online { "y" } else { "n" };
        self.add_tag(TAG_KEY_ONLINE, online_value);
        self.add_tag(TAG_KEY_STAT_ID, stat_id);
    }
}
