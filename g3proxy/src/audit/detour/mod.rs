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
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::oneshot;

use g3_daemon::server::ServerQuitPolicy;
use g3_dpi::Protocol;
use g3_types::net::UpstreamAddr;

use crate::config::audit::AuditStreamDetourConfig;
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectTaskNotes;
use crate::serve::{ServerTaskError, ServerTaskResult};

mod connect;
use connect::{StreamDetourConnector, StreamDetourRequest};

mod pool;
use pool::{StreamDetourPool, StreamDetourPoolHandle};

mod stream;
use stream::StreamDetourStream;

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

    pub(crate) async fn detour_relay<CR, CW, UR, UW, SC>(
        &self,
        clt_r: CR,
        clt_w: CW,
        ups_r: UR,
        ups_w: UW,
        ctx: StreamDetourContext<'_, SC>,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
        SC: ServerConfig,
    {
        let (sender, receiver) = oneshot::channel();
        let req = StreamDetourRequest(sender);

        if let Err(e) = self.req_sender.try_send(req) {
            match e {
                flume::TrySendError::Full(req) => {
                    self.pool_handle.request_new_connection();
                    if self.req_sender.send_async(req).await.is_err() {
                        return Err(ServerTaskError::InternalAdapterError(anyhow!(
                            "stream detour client is down"
                        )));
                    }
                }
                flume::TrySendError::Disconnected(_req) => {
                    return Err(ServerTaskError::InternalAdapterError(anyhow!(
                        "stream detour client is down"
                    )));
                }
            }
        }

        let detour_stream = receiver.await.map_err(|e| {
            ServerTaskError::InternalAdapterError(anyhow!("failed to get detour stream: {e}"))
        })?;

        ctx.relay(clt_r, clt_w, ups_r, ups_w, detour_stream).await
    }
}
