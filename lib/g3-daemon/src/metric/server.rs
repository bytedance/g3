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

use cadence::{Metric, MetricBuilder};

use g3_types::metrics::{MetricsName, StaticMetricsTags};

use super::TAG_KEY_STAT_ID;

pub const TAG_KEY_SERVER: &str = "server";
pub const TAG_KEY_ONLINE: &str = "online";

pub trait ServerMetricExt<'m> {
    fn add_server_tags(
        self,
        server: &'m MetricsName,
        online_value: &'m str,
        stat_id: &'m str,
    ) -> Self;
    fn add_server_extra_tags(self, tags: &'m Option<Arc<StaticMetricsTags>>) -> Self;
}

impl<'m, 'c, T> ServerMetricExt<'m> for MetricBuilder<'m, 'c, T>
where
    T: Metric + From<String>,
{
    fn add_server_tags(
        self,
        server: &'m MetricsName,
        online_value: &'m str,
        stat_id: &'m str,
    ) -> Self {
        self.with_tag(TAG_KEY_SERVER, server.as_str())
            .with_tag(TAG_KEY_ONLINE, online_value)
            .with_tag(TAG_KEY_STAT_ID, stat_id)
    }

    fn add_server_extra_tags(mut self, tags: &'m Option<Arc<StaticMetricsTags>>) -> Self {
        if let Some(tags) = tags {
            for (k, v) in tags.iter() {
                self = self.with_tag(k.as_str(), v.as_str());
            }
        }
        self
    }
}
