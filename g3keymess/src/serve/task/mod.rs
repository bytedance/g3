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
use std::sync::Arc;

use anyhow::anyhow;
use slog::Logger;
use tokio::io::AsyncRead;
use tokio::sync::{broadcast, Semaphore};

use crate::config::server::KeyServerConfig;
use crate::protocol::KeylessRequest;
use crate::serve::{KeyServerStats, ServerReloadCommand};

mod multiplex;
mod simplex;

#[allow(unused)]
pub(crate) struct KeylessTaskContext {
    pub(crate) server_config: Arc<KeyServerConfig>,
    pub(crate) server_stats: Arc<KeyServerStats>,
    pub(crate) peer_addr: SocketAddr,
    pub(crate) local_addr: SocketAddr,
    pub(crate) task_logger: Logger,
    pub(crate) request_logger: Logger,
    pub(crate) reload_notifier: broadcast::Receiver<ServerReloadCommand>,
    pub(crate) concurrency_limit: Option<Arc<Semaphore>>,
}

pub(crate) struct KeylessTask {
    ctx: KeylessTaskContext,
    buf: Vec<u8>,
}

impl Drop for KeylessTask {
    fn drop(&mut self) {
        self.ctx.server_stats.dec_alive_task();
    }
}

impl KeylessTask {
    pub(crate) fn new(ctx: KeylessTaskContext) -> Self {
        ctx.server_stats.add_task();
        ctx.server_stats.inc_alive_task();
        KeylessTask {
            ctx,
            buf: Vec::with_capacity(crate::protocol::MESSAGE_PADDED_LENGTH + 2),
        }
    }

    async fn timed_read_request<R>(&mut self, reader: &mut R) -> anyhow::Result<KeylessRequest>
    where
        R: AsyncRead + Unpin,
    {
        match tokio::time::timeout(
            self.ctx.server_config.request_read_timeout,
            KeylessRequest::read(reader, &mut self.buf),
        )
        .await
        {
            Ok(Ok(req)) => Ok(req),
            Ok(Err(e)) => Err(anyhow!("request read failed: {e}")),
            Err(_) => Err(anyhow!("request read timeout")),
        }
    }
}
