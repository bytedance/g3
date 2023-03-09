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

use std::net::SocketAddr;
use std::ops::Deref;
use std::time::Duration;

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use tokio::time::Instant;
use uuid::{v1::Context, Uuid};

use g3_types::limit::GaugeSemaphorePermit;
use g3_types::route::EgressPathSelection;

use crate::auth::UserContext;

#[derive(Clone)]
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
    #[allow(unused)]
    pub(crate) worker_id: Option<usize>,
    pub(crate) client_addr: SocketAddr,
    pub(crate) server_addr: SocketAddr,
    pub(crate) stage: ServerTaskStage,
    pub(crate) start_at: DateTime<Utc>,
    create_ins: Instant,
    pub(crate) id: Uuid,
    user_ctx: Option<UserContext>,
    pub(crate) wait_time: Duration,
    pub(crate) ready_time: Duration,
    pub(crate) egress_path_selection: EgressPathSelection,
    /// the following fields should not be cloned
    pub(crate) user_req_alive_permit: Option<GaugeSemaphorePermit>,
}

impl ServerTaskNotes {
    fn generate_uuid(time: &DateTime<Utc>) -> Uuid {
        static UUID_CONTEXT: Lazy<Context> = Lazy::new(|| {
            use rand::Rng;

            let mut rng = rand::thread_rng();
            Context::new(rng.gen())
        });
        static UUID_NODE_ID: Lazy<[u8; 6]> = Lazy::new(|| {
            use rand::RngCore;

            let mut bytes = [0u8; 6];
            let mut rng = rand::thread_rng();
            rng.fill_bytes(&mut bytes);
            bytes
        });

        Uuid::new_v1(
            uuid::Timestamp::from_unix(
                &*UUID_CONTEXT,
                time.timestamp() as u64,
                time.timestamp_subsec_nanos().max(999_999_999), // ignore leap second
            ),
            UUID_NODE_ID.deref(),
        )
    }

    pub(crate) fn new(
        worker_id: Option<usize>,
        client_addr: SocketAddr,
        server_addr: SocketAddr,
        user_ctx: Option<UserContext>,
        wait_time: Duration,
        egress_path_selection: EgressPathSelection,
    ) -> Self {
        let started = Utc::now();
        let uuid = ServerTaskNotes::generate_uuid(&started);
        ServerTaskNotes {
            worker_id,
            client_addr,
            server_addr,
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
    pub(crate) fn user_ctx(&self) -> Option<&UserContext> {
        self.user_ctx.as_ref()
    }

    #[inline]
    pub(crate) fn user_ctx_mut(&mut self) -> Option<&mut UserContext> {
        self.user_ctx.as_mut()
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
