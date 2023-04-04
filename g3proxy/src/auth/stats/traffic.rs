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

use arc_swap::ArcSwapOption;

use g3_daemon::stat::remote::*;
use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_types::stats::StatId;

use crate::auth::UserType;
use crate::stat::types::{
    TrafficSnapshot, TrafficStats, UpstreamTrafficSnapshot, UpstreamTrafficStats,
};

pub(crate) struct UserTrafficStats {
    id: StatId,
    user_group: MetricsName,
    user: String,
    user_type: UserType,
    server: String,
    server_extra_tags: Arc<ArcSwapOption<StaticMetricsTags>>,
    pub(crate) io: TrafficStats,
}

#[derive(Default)]
pub(crate) struct UserTrafficSnapshot {
    pub(crate) io: TrafficSnapshot,
}

impl UserTrafficStats {
    pub(crate) fn new(
        user_group: &MetricsName,
        user: &str,
        user_type: UserType,
        server: &str,
        server_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
    ) -> Self {
        UserTrafficStats {
            id: StatId::new(),
            user_group: user_group.clone(),
            user: user.to_string(),
            user_type,
            server: server.to_string(),
            server_extra_tags: Arc::clone(server_extra_tags),
            io: Default::default(),
        }
    }

    #[inline]
    pub(crate) fn stat_id(&self) -> StatId {
        self.id
    }

    #[inline]
    pub(crate) fn user_group(&self) -> &MetricsName {
        &self.user_group
    }

    #[inline]
    pub(crate) fn user(&self) -> &str {
        &self.user
    }

    #[inline]
    pub(crate) fn user_type(&self) -> &str {
        self.user_type.as_str()
    }

    #[inline]
    pub(crate) fn server(&self) -> &str {
        &self.server
    }

    pub(crate) fn server_extra_tags(&self) -> Option<Arc<StaticMetricsTags>> {
        let guard = self.server_extra_tags.load();
        (*guard).as_ref().map(Arc::clone)
    }
}

pub(crate) struct UserUpstreamTrafficStats {
    id: StatId,
    user_group: MetricsName,
    user: String,
    user_type: UserType,
    escaper: String,
    escaper_extra_tags: Arc<ArcSwapOption<StaticMetricsTags>>,
    pub(crate) io: UpstreamTrafficStats,
}

#[derive(Default)]
pub(crate) struct UserUpstreamTrafficSnapshot {
    pub(crate) io: UpstreamTrafficSnapshot,
}

impl UserUpstreamTrafficStats {
    pub(crate) fn new(
        user_group: &MetricsName,
        user: &str,
        user_type: UserType,
        escaper: &str,
        escaper_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
    ) -> Self {
        UserUpstreamTrafficStats {
            id: StatId::new(),
            user_group: user_group.clone(),
            user: user.to_string(),
            user_type,
            escaper: escaper.to_string(),
            escaper_extra_tags: Arc::clone(escaper_extra_tags),
            io: Default::default(),
        }
    }

    #[inline]
    pub(crate) fn stat_id(&self) -> StatId {
        self.id
    }

    #[inline]
    pub(crate) fn user_group(&self) -> &MetricsName {
        &self.user_group
    }

    #[inline]
    pub(crate) fn user(&self) -> &str {
        &self.user
    }

    #[inline]
    pub(crate) fn user_type(&self) -> &str {
        self.user_type.as_str()
    }

    #[inline]
    pub(crate) fn escaper(&self) -> &str {
        &self.escaper
    }

    pub(crate) fn escaper_extra_tags(&self) -> Option<Arc<StaticMetricsTags>> {
        let guard = self.escaper_extra_tags.load();
        (*guard).as_ref().map(Arc::clone)
    }
}

impl TcpConnectionTaskRemoteStats for UserUpstreamTrafficStats {
    fn add_read_bytes(&self, size: u64) {
        self.io.tcp.add_in_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.io.tcp.add_out_bytes(size);
    }
}
