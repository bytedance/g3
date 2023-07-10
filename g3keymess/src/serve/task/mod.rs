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
use tokio::io::AsyncRead;
use tokio::sync::{broadcast, Semaphore};

use g3_daemon::server::ServerQuitPolicy;

use crate::config::server::KeyServerConfig;
use crate::protocol::KeylessRequest;
use crate::serve::{KeyServer, KeyServerStats, ServerReloadCommand};

mod multiplex;
mod simplex;

pub(crate) struct KeylessTask {
    server_config: Arc<KeyServerConfig>,
    server_stats: Arc<KeyServerStats>,
    #[allow(unused)]
    peer_addr: SocketAddr,
    #[allow(unused)]
    local_addr: SocketAddr,
    reload_notifier: broadcast::Receiver<ServerReloadCommand>,
    #[allow(unused)]
    server_quit_policy: Arc<ServerQuitPolicy>,
    buf: Vec<u8>,
    concurrency_limit: Option<Arc<Semaphore>>,
}

impl Drop for KeylessTask {
    fn drop(&mut self) {
        self.server_stats.dec_alive_task();
    }
}

impl KeylessTask {
    pub(crate) fn new(server: &KeyServer, peer_addr: SocketAddr, local_addr: SocketAddr) -> Self {
        let server_stats = server.get_server_stats();
        server_stats.add_task();
        server_stats.inc_alive_task();
        KeylessTask {
            server_config: server.clone_config(),
            server_stats: server.get_server_stats(),
            peer_addr,
            local_addr,
            reload_notifier: server.reload_notifier(),
            server_quit_policy: server.quit_policy().clone(),
            buf: Vec::with_capacity(crate::protocol::MESSAGE_PADDED_LENGTH + 2),
            concurrency_limit: server.concurrency_limit(),
        }
    }

    async fn timed_read_request<R>(&mut self, reader: &mut R) -> anyhow::Result<KeylessRequest>
    where
        R: AsyncRead + Unpin,
    {
        match tokio::time::timeout(
            self.server_config.request_read_timeout,
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
