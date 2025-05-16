/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::{Arc, Mutex};

use arc_swap::ArcSwapOption;
use foldhash::HashMap;

use g3_types::metrics::{MetricTagMap, NodeName};

use super::{UserRequestStats, UserTrafficStats, UserUpstreamTrafficStats};
use crate::auth::UserType;

pub(crate) struct UserSiteStats {
    user: Arc<str>,
    user_group: NodeName,
    site_id: NodeName,
    pub(crate) request: Mutex<HashMap<NodeName, Arc<UserRequestStats>>>,
    pub(crate) client_io: Mutex<HashMap<NodeName, Arc<UserTrafficStats>>>,
    pub(crate) remote_io: Mutex<HashMap<NodeName, Arc<UserUpstreamTrafficStats>>>,
}

impl UserSiteStats {
    pub(crate) fn new(user: Arc<str>, user_group: &NodeName, site_id: &NodeName) -> Self {
        UserSiteStats {
            user,
            user_group: user_group.clone(),
            site_id: site_id.clone(),
            request: Mutex::new(HashMap::default()),
            client_io: Mutex::new(HashMap::default()),
            remote_io: Mutex::new(HashMap::default()),
        }
    }

    #[inline]
    pub(crate) fn user_group(&self) -> &NodeName {
        &self.user_group
    }

    #[inline]
    pub(crate) fn user(&self) -> &str {
        &self.user
    }

    pub(crate) fn fetch_request_stats(
        &self,
        user_type: UserType,
        server: &NodeName,
        server_extra_tags: &Arc<ArcSwapOption<MetricTagMap>>,
    ) -> Arc<UserRequestStats> {
        let mut new_stats = None;

        let mut map = self.request.lock().unwrap();
        let stats = map
            .entry(server.clone())
            .or_insert_with(|| {
                let stats = Arc::new(UserRequestStats::new(
                    &self.user_group,
                    self.user.clone(),
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
        server: &NodeName,
        server_extra_tags: &Arc<ArcSwapOption<MetricTagMap>>,
    ) -> Arc<UserTrafficStats> {
        let mut new_stats = None;

        let mut map = self.client_io.lock().unwrap();
        let stats = map
            .entry(server.clone())
            .or_insert_with(|| {
                let stats = Arc::new(UserTrafficStats::new(
                    &self.user_group,
                    self.user.clone(),
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
        escaper: &NodeName,
        escaper_extra_tags: &Arc<ArcSwapOption<MetricTagMap>>,
    ) -> Arc<UserUpstreamTrafficStats> {
        let mut new_stats = None;

        let mut map = self.remote_io.lock().unwrap();
        let stats = map
            .entry(escaper.clone())
            .or_insert_with(|| {
                let stats = Arc::new(UserUpstreamTrafficStats::new(
                    &self.user_group,
                    self.user.clone(),
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
