/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use anyhow::anyhow;
use tokio::sync::oneshot;

use g3_daemon::server::ServerQuitPolicy;
use g3_dpi::Protocol;
use g3_types::net::UpstreamAddr;

use crate::config::audit::AuditStreamDetourConfig;
use crate::inspect::StreamInspectTaskNotes;

mod connect;
use connect::{StreamDetourConnector, StreamDetourRequest};

mod pool;
use pool::{StreamDetourPool, StreamDetourPoolHandle};

mod stream;
use stream::StreamDetourStream;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub(crate) enum DetourAction {
    Continue,
    Bypass,
    Block,
}

impl From<u16> for DetourAction {
    fn from(value: u16) -> Self {
        match value {
            0 => DetourAction::Continue,
            1 => DetourAction::Bypass,
            _ => DetourAction::Block,
        }
    }
}

pub(crate) struct StreamDetourContext<'a, SC> {
    server_config: &'a Arc<SC>,
    server_quit_policy: &'a Arc<ServerQuitPolicy>,
    task_notes: &'a StreamInspectTaskNotes,
    upstream: &'a UpstreamAddr,
    protocol: Protocol,
    payload: Vec<u8>,
}

impl<'a, SC> StreamDetourContext<'a, SC> {
    pub(crate) fn new(
        server_config: &'a Arc<SC>,
        server_quit_policy: &'a Arc<ServerQuitPolicy>,
        task_notes: &'a StreamInspectTaskNotes,
        upstream: &'a UpstreamAddr,
        protocol: Protocol,
    ) -> Self {
        StreamDetourContext {
            server_config,
            server_quit_policy,
            task_notes,
            upstream,
            protocol,
            payload: Vec::new(),
        }
    }

    pub(crate) fn set_payload(&mut self, payload: Vec<u8>) {
        self.payload = payload;
    }
}

pub(crate) struct StreamDetourClient {
    req_sender: flume::Sender<StreamDetourRequest>,
    pool_handle: StreamDetourPoolHandle,
}

impl StreamDetourClient {
    pub(super) fn new(config: Arc<AuditStreamDetourConfig>) -> anyhow::Result<Self> {
        let (req_sender, req_receiver) = flume::unbounded();
        let connector = StreamDetourConnector::new(config.clone())?;
        let pool_handle =
            StreamDetourPool::spawn(config.connection_pool, req_receiver, Arc::new(connector));
        Ok(StreamDetourClient {
            req_sender,
            pool_handle,
        })
    }

    pub(crate) async fn open_detour_stream(&self) -> anyhow::Result<StreamDetourStream> {
        let (sender, receiver) = oneshot::channel();
        let req = StreamDetourRequest(sender);

        if let Err(e) = self.req_sender.try_send(req) {
            match e {
                flume::TrySendError::Full(req) => {
                    self.pool_handle.request_new_connection();
                    if self.req_sender.send_async(req).await.is_err() {
                        return Err(anyhow!("stream detour client is down"));
                    }
                }
                flume::TrySendError::Disconnected(_req) => {
                    return Err(anyhow!("stream detour client is down"));
                }
            }
        }

        // TODO add timeout limit
        receiver
            .await
            .map_err(|e| anyhow!("failed to get detour stream: {e}"))
    }
}
