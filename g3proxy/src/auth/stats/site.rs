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
use arc_swap::ArcSwapOption;

use g3_types::metrics::{MetricsName, StaticMetricsTags};

use super::{UserRequestStats, UserTrafficStats, UserUpstreamTrafficStats};
use crate::auth::UserType;

pub(crate) struct UserSiteStats {
    user: String,
    user_group: MetricsName,
    site_id: MetricsName,
    pub(crate) request: Mutex<AHashMap<String, Arc<UserRequestStats>>>,
    pub(crate) client_io: Mutex<AHashMap<String, Arc<UserTrafficStats>>>,
    pub(crate) remote_io: Mutex<AHashMap<String, Arc<UserUpstreamTrafficStats>>>,
}

impl UserSiteStats {
    pub(crate) fn new(user: &str, user_group: &MetricsName, site_id: &MetricsName) -> Self {
        UserSiteStats {
            user: user.to_string(),
            user_group: user_group.clone(),
            site_id: site_id.clone(),
            request: Mutex::new(AHashMap::new()),
            client_io: Mutex::new(AHashMap::new()),
            remote_io: Mutex::new(AHashMap::new()),
        }
    }

    pub(crate) fn fetch_request_stats(
        &self,
        user_type: UserType,
        server: &MetricsName,
        server_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
    ) -> Arc<UserRequestStats> {
        let mut new_stats = None;

        let mut map = self.request.lock().unwrap();
        let stats = map
            .entry(server.to_string())
            .or_insert_with(|| {
                let stats = Arc::new(UserRequestStats::new(
                    &self.user_group,
                    &self.user,
                    user_type,
                    server,
                    server_extra_tags,
                ));
                new_stats = Some(stats.clone());
                stats
            })
            .clone();
        drop(map);

        if let Some(stats) = new_stats {
            crate::stat::user_site::push_request_stats(stats, &self.site_id);
        }

        stats
    }

    pub(crate) fn fetch_traffic_stats(
        &self,
        user_type: UserType,
        server: &MetricsName,
        server_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
    ) -> Arc<UserTrafficStats> {
        let mut new_stats = None;

        let mut map = self.client_io.lock().unwrap();
        let stats = map
            .entry(server.to_string())
            .or_insert_with(|| {
                let stats = Arc::new(UserTrafficStats::new(
                    &self.user_group,
                    &self.user,
                    user_type,
                    server,
                    server_extra_tags,
                ));
                new_stats = Some(stats.clone());
                stats
            })
            .clone();
        drop(map);

        if let Some(stats) = new_stats {
            crate::stat::user_site::push_traffic_stats(stats, &self.site_id);
        }

        stats
    }

    pub(crate) fn fetch_upstream_traffic_stats(
        &self,
        user_type: UserType,
        escaper: &MetricsName,
        escaper_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
    ) -> Arc<UserUpstreamTrafficStats> {
        let mut new_stats = None;

        let mut map = self.remote_io.lock().unwrap();
        let stats = map
            .entry(escaper.to_string())
            .or_insert_with(|| {
                let stats = Arc::new(UserUpstreamTrafficStats::new(
                    &self.user_group,
                    &self.user,
                    user_type,
                    escaper,
                    escaper_extra_tags,
                ));
                new_stats = Some(stats.clone());
                stats
            })
            .clone();
        drop(map);

        if let Some(stats) = new_stats {
            crate::stat::user_site::push_upstream_traffic_stats(stats, &self.site_id);
        }

        stats
    }
}
