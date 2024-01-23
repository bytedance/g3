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

use g3_daemon::metrics::TAG_KEY_STAT_ID;
use g3_statsd_client::{StatsdClient, StatsdTagGroup};
use g3_types::metrics::MetricsName;
use g3_types::stats::StatId;

pub(crate) mod stream;

const TAG_KEY_BACKEND: &str = "backend";

trait BackendMetricExt {
    fn add_backend_tags(&mut self, backend: &MetricsName, stat_id: StatId);
}

impl BackendMetricExt for StatsdTagGroup {
    fn add_backend_tags(&mut self, backend: &MetricsName, stat_id: StatId) {
        let mut buffer = itoa::Buffer::new();
        let stat_id = buffer.format(stat_id.as_u64());
        self.add_tag(TAG_KEY_BACKEND, backend);
        self.add_tag(TAG_KEY_STAT_ID, stat_id);
    }
}

pub(in crate::stat) fn sync_stats() {
    stream::sync_stats();
}

pub(in crate::stat) fn emit_stats(client: &mut StatsdClient) {
    stream::emit_stats(client);
}
