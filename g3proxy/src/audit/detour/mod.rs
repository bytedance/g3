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
use std::time::Duration;

use anyhow::anyhow;
use tokio::sync::oneshot;

use g3_daemon::server::ServerQuitPolicy;
use g3_dpi::Protocol;
use g3_io_ext::IdleWheel;
use g3_types::net::UpstreamAddr;

use crate::config::audit::AuditStreamDetourConfig;
use crate::config::server::ServerConfig;
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
    idle_wheel: &'a Arc<IdleWheel>,
    task_notes: &'a StreamInspectTaskNotes,
    upstream: &'a UpstreamAddr,
    protocol: Protocol,
    payload: Vec<u8>,
    request_timeout: Duration,
    max_idle_count: usize,
}

impl<SC> StreamDetourContext<'_, SC> {
    pub(crate) fn set_payload(&mut self, payload: Vec<u8>) {
        self.payload = payload;
    }
}

pub(crate) struct StreamDetourClient {
    config: Arc<AuditStreamDetourConfig>,
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
            config,
            req_sender,
            pool_handle,
        })
    }

    pub(crate) fn build_context<'a, SC>(
        &self,
        server_config: &'a Arc<SC>,
        server_quit_policy: &'a Arc<ServerQuitPolicy>,
        idle_wheel: &'a Arc<IdleWheel>,
        task_notes: &'a StreamInspectTaskNotes,
        upstream: &'a UpstreamAddr,
        protocol: Protocol,
    ) -> StreamDetourContext<'a, SC>
    where
        SC: ServerConfig,
    {
        let max_idle_count = task_notes
            .user()
            .and_then(|u| u.task_max_idle_count())
            .unwrap_or(server_config.task_max_idle_count());
        StreamDetourContext {
            server_config,
            server_quit_policy,
            idle_wheel,
            task_notes,
            upstream,
            protocol,
            payload: Vec::new(),
            request_timeout: self.config.request_timeout,
            max_idle_count,
        }
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

        match tokio::time::timeout(self.config.stream_open_timeout, receiver).await {
            Ok(Ok(s)) => Ok(s),
            Ok(Err(e)) => Err(anyhow!("failed to open detour stream: {e}")),
            Err(_) => Err(anyhow!("timed out to open detour stream")),
        }
    }
}
