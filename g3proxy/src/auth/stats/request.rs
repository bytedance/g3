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

use g3_types::metrics::StaticMetricsTags;
use g3_types::stats::StatId;

use crate::auth::UserType;
use crate::stat::types::{
    ConnectionSnapshot, ConnectionStats, KeepaliveRequestSnapshot, KeepaliveRequestStats,
    L7ConnectionAliveStats, RequestAliveStats, RequestSnapshot, RequestStats,
};

pub(crate) struct UserRequestStats {
    id: StatId,
    user_group: String,
    user: String,
    user_type: UserType,
    server: String,
    server_extra_tags: Arc<ArcSwapOption<StaticMetricsTags>>,
    pub(crate) conn_total: ConnectionStats,
    pub(crate) req_total: RequestStats,
    pub(crate) req_alive: RequestAliveStats,
    pub(crate) req_ready: RequestStats,
    pub(crate) req_reuse: KeepaliveRequestStats,
    pub(crate) req_renew: KeepaliveRequestStats,
    pub(crate) l7_conn_alive: L7ConnectionAliveStats,
}

#[derive(Default)]
pub(crate) struct UserRequestSnapshot {
    pub(crate) conn_total: ConnectionSnapshot,
    pub(crate) req_total: RequestSnapshot,
    pub(crate) req_ready: RequestSnapshot,
    pub(crate) req_reuse: KeepaliveRequestSnapshot,
    pub(crate) req_renew: KeepaliveRequestSnapshot,
}

impl UserRequestStats {
    pub(crate) fn new(
        user_group: &str,
        user: &str,
        user_type: UserType,
        server: &str,
        server_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
    ) -> Self {
        UserRequestStats {
            id: StatId::new(),
            user_group: user_group.to_string(),
            user: user.to_string(),
            user_type,
            server: server.to_string(),
            server_extra_tags: Arc::clone(server_extra_tags),
            conn_total: Default::default(),
            req_total: Default::default(),
            req_alive: Default::default(),
            req_ready: Default::default(),
            req_reuse: Default::default(),
            req_renew: Default::default(),
            l7_conn_alive: Default::default(),
        }
    }

    #[inline]
    pub(crate) fn stat_id(&self) -> StatId {
        self.id
    }

    #[inline]
    pub(crate) fn user_group(&self) -> &str {
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
