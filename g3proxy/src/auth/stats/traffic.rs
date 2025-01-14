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
use g3_types::metrics::{NodeName, StaticMetricsTags};
use g3_types::stats::StatId;

use crate::auth::UserType;
use crate::stat::types::{
    TrafficSnapshot, TrafficStats, UpstreamTrafficSnapshot, UpstreamTrafficStats,
};

pub(crate) struct UserTrafficStats {
    id: StatId,
    user_group: NodeName,
    user: Arc<str>,
    user_type: UserType,
    server: NodeName,
    server_extra_tags: Arc<ArcSwapOption<StaticMetricsTags>>,
    pub(crate) io: TrafficStats,
}

#[derive(Default)]
pub(crate) struct UserTrafficSnapshot {
    pub(crate) io: TrafficSnapshot,
}

impl UserTrafficStats {
    pub(crate) fn new(
        user_group: &NodeName,
        user: Arc<str>,
        user_type: UserType,
        server: &NodeName,
        server_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
    ) -> Self {
        UserTrafficStats {
            id: StatId::new(),
            user_group: user_group.clone(),
            user,
            user_type,
            server: server.clone(),
            server_extra_tags: Arc::clone(server_extra_tags),
            io: Default::default(),
        }
    }

    #[inline]
    pub(crate) fn stat_id(&self) -> StatId {
        self.id
    }

    #[inline]
    pub(crate) fn user_group(&self) -> &NodeName {
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
    pub(crate) fn server(&self) -> &NodeName {
        &self.server
    }

    #[inline]
    pub(crate) fn server_extra_tags(&self) -> Option<Arc<StaticMetricsTags>> {
        self.server_extra_tags.load_full()
    }
}

pub(crate) struct UserUpstreamTrafficStats {
    id: StatId,
    user_group: NodeName,
    user: Arc<str>,
    user_type: UserType,
    escaper: NodeName,
    escaper_extra_tags: Arc<ArcSwapOption<StaticMetricsTags>>,
    pub(crate) io: UpstreamTrafficStats,
}

#[derive(Default)]
pub(crate) struct UserUpstreamTrafficSnapshot {
    pub(crate) io: UpstreamTrafficSnapshot,
}

impl UserUpstreamTrafficStats {
    pub(crate) fn new(
        user_group: &NodeName,
        user: Arc<str>,
        user_type: UserType,
        escaper: &NodeName,
        escaper_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
    ) -> Self {
        UserUpstreamTrafficStats {
            id: StatId::new(),
            user_group: user_group.clone(),
            user,
            user_type,
            escaper: escaper.clone(),
            escaper_extra_tags: Arc::clone(escaper_extra_tags),
            io: Default::default(),
        }
    }

    #[inline]
    pub(crate) fn stat_id(&self) -> StatId {
        self.id
    }

    #[inline]
    pub(crate) fn user_group(&self) -> &NodeName {
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
    pub(crate) fn escaper(&self) -> &NodeName {
        &self.escaper
    }

    #[inline]
    pub(crate) fn escaper_extra_tags(&self) -> Option<Arc<StaticMetricsTags>> {
        self.escaper_extra_tags.load_full()
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
