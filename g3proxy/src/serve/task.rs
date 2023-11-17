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

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::time::Instant;
use uuid::Uuid;

use g3_daemon::server::ClientConnectionInfo;
use g3_types::limit::GaugeSemaphorePermit;
use g3_types::route::EgressPathSelection;

use crate::auth::UserContext;

static DEFAULT_PATH_SELECTION: OnceLock<Arc<EgressPathSelection>> = OnceLock::new();

#[derive(Clone, Copy)]
pub(crate) enum ServerTaskStage {
    Created,
    Preparing,
    Connecting,
    Connected,
    Replying,
    LoggedIn,
    Relaying,
    Finished,
}

impl ServerTaskStage {
    pub(crate) fn brief(&self) -> &'static str {
        match self {
            ServerTaskStage::Created => "Created",
            ServerTaskStage::Preparing => "Preparing",
            ServerTaskStage::Connecting => "Connecting",
            ServerTaskStage::Connected => "Connected",
            ServerTaskStage::Replying => "Replying",
            ServerTaskStage::LoggedIn => "LoggedIn",
            ServerTaskStage::Relaying => "Relaying",
            ServerTaskStage::Finished => "Finished",
        }
    }
}

/// server task notes is bounded to a single client connection.
/// it can be reset if the connection is consisted of many tasks.
/// Do not share this struct between different client connections.
pub(crate) struct ServerTaskNotes {
    cc_info: ClientConnectionInfo,
    pub(crate) stage: ServerTaskStage,
    pub(crate) start_at: DateTime<Utc>,
    create_ins: Instant,
    pub(crate) id: Uuid,
    user_ctx: Option<UserContext>,
    pub(crate) wait_time: Duration,
    pub(crate) ready_time: Duration,
    pub(crate) egress_path_selection: Arc<EgressPathSelection>,
    /// the following fields should not be cloned
    pub(crate) user_req_alive_permit: Option<GaugeSemaphorePermit>,
}

impl ServerTaskNotes {
    pub(crate) fn new(
        cc_info: ClientConnectionInfo,
        user_ctx: Option<UserContext>,
        wait_time: Duration,
    ) -> Self {
        let path_selection =
            DEFAULT_PATH_SELECTION.get_or_init(|| Arc::new(EgressPathSelection::Default));
        ServerTaskNotes::with_path_selection(cc_info, user_ctx, wait_time, path_selection.clone())
    }

    pub(crate) fn with_path_selection(
        cc_info: ClientConnectionInfo,
        user_ctx: Option<UserContext>,
        wait_time: Duration,
        egress_path_selection: Arc<EgressPathSelection>,
    ) -> Self {
        let started = Utc::now();
        let uuid = g3_daemon::server::task::generate_uuid(&started);
        ServerTaskNotes {
            cc_info,
            stage: ServerTaskStage::Created,
            start_at: started,
            create_ins: Instant::now(),
            id: uuid,
            user_ctx,
            wait_time,
            ready_time: Duration::default(),
            egress_path_selection,
            user_req_alive_permit: None,
        }
    }

    #[inline]
    pub(crate) fn client_addr(&self) -> SocketAddr {
        self.cc_info.client_addr()
    }

    #[inline]
    pub(crate) fn client_ip(&self) -> IpAddr {
        self.cc_info.client_ip()
    }

    #[inline]
    pub(crate) fn server_addr(&self) -> SocketAddr {
        self.cc_info.server_addr()
    }

    #[inline]
    pub(crate) fn user_ctx(&self) -> Option<&UserContext> {
        self.user_ctx.as_ref()
    }

    #[inline]
    pub(crate) fn user_ctx_mut(&mut self) -> Option<&mut UserContext> {
        self.user_ctx.as_mut()
    }

    pub(crate) fn raw_user_name(&self) -> Option<&str> {
        self.user_ctx.as_ref().and_then(|c| c.raw_user_name())
    }

    #[inline]
    pub(crate) fn task_created_instant(&self) -> Instant {
        self.create_ins
    }

    #[inline]
    pub(crate) fn time_elapsed(&self) -> Duration {
        self.create_ins.elapsed()
    }

    pub(crate) fn mark_relaying(&mut self) {
        self.stage = ServerTaskStage::Relaying;
        self.ready_time = self.create_ins.elapsed();
    }
}
