/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use g3_daemon::metrics::TAG_KEY_STAT_ID;
use g3_statsd_client::{StatsdClient, StatsdTagGroup};
use g3_types::metrics::NodeName;
use g3_types::stats::StatId;

pub(crate) mod keyless;
pub(crate) mod stream;

const TAG_KEY_BACKEND: &str = "backend";

trait BackendMetricExt {
    fn add_backend_tags(&mut self, backend: &NodeName, stat_id: StatId);
}

impl BackendMetricExt for StatsdTagGroup {
    fn add_backend_tags(&mut self, backend: &NodeName, stat_id: StatId) {
        let mut buffer = itoa::Buffer::new();
        let stat_id = buffer.format(stat_id.as_u64());
        self.add_tag(TAG_KEY_BACKEND, backend);
        self.add_tag(TAG_KEY_STAT_ID, stat_id);
    }
}

pub(in crate::stat) fn sync_stats() {
    stream::sync_stats();
    keyless::sync_stats();
}

pub(in crate::stat) fn emit_stats(client: &mut StatsdClient) {
    stream::emit_stats(client);
    keyless::emit_stats(client);
}
