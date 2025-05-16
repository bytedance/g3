/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_statsd_client::StatsdTagGroup;
use g3_types::metrics::NodeName;
use g3_types::stats::StatId;

use super::TAG_KEY_STAT_ID;

pub const TAG_KEY_SERVER: &str = "server";
pub const TAG_KEY_ONLINE: &str = "online";

pub trait ServerMetricExt {
    fn add_server_tags(&mut self, server: &NodeName, online: bool, stat_id: StatId);
}

impl ServerMetricExt for StatsdTagGroup {
    fn add_server_tags(&mut self, server: &NodeName, online: bool, stat_id: StatId) {
        let mut buffer = itoa::Buffer::new();
        let stat_id = buffer.format(stat_id.as_u64());

        self.add_tag(TAG_KEY_SERVER, server);

        let online_value = if online { "y" } else { "n" };
        self.add_tag(TAG_KEY_ONLINE, online_value);
        self.add_tag(TAG_KEY_STAT_ID, stat_id);
    }
}
