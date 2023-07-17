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

use chrono::{DateTime, Utc};
use slog::{slog_info, Logger};
use tokio::io::AsyncRead;
use tokio::sync::{broadcast, Semaphore};
use uuid::Uuid;

use g3_slog_types::{LtDateTime, LtUuid};

use crate::config::server::KeyServerConfig;
use crate::protocol::{KeylessAction, KeylessRequest};
use crate::serve::{KeyServerRequestStats, KeyServerStats, ServerReloadCommand, ServerTaskError};

mod multiplex;
mod simplex;

struct WrappedKeylessRequest {
    inner: KeylessRequest,
    stats: Arc<KeyServerRequestStats>,
}

impl WrappedKeylessRequest {
    fn new(req: KeylessRequest, server_stats: &Arc<KeyServerStats>) -> Self {
        let stats = match req.action {
            KeylessAction::Ping => server_stats.ping_pong.clone(),
            KeylessAction::RsaDecrypt(_) => server_stats.rsa_decrypt.clone(),
            KeylessAction::RsaSign(_) => server_stats.rsa_sign.clone(),
            KeylessAction::RsaPssSign(_) => server_stats.rsa_pss_sign.clone(),
            KeylessAction::EcdsaSign(_) => server_stats.ecdsa_sign.clone(),
            KeylessAction::Ed25519Sign => server_stats.ed25519_sign.clone(),
            KeylessAction::NotSet => unreachable!(),
        };
        stats.add_total();
        stats.inc_alive();
        WrappedKeylessRequest { inner: req, stats }
    }
}

impl Drop for WrappedKeylessRequest {
    fn drop(&mut self) {
        self.stats.dec_alive();
    }
}

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
    id: Uuid,
    ctx: KeylessTaskContext,
    started: DateTime<Utc>,
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

        let started = Utc::now();

        KeylessTask {
            id: g3_daemon::server::task::generate_uuid(&started),
            ctx,
            started,
            buf: Vec::with_capacity(crate::protocol::MESSAGE_PADDED_LENGTH + 2),
        }
    }

    async fn timed_read_request<R>(
        &mut self,
        reader: &mut R,
    ) -> Result<WrappedKeylessRequest, ServerTaskError>
    where
        R: AsyncRead + Unpin,
    {
        match tokio::time::timeout(
            self.ctx.server_config.request_read_timeout,
            KeylessRequest::read(reader, &mut self.buf),
        )
        .await
        {
            Ok(Ok(req)) => Ok(WrappedKeylessRequest::new(req, &self.ctx.server_stats)),
            Ok(Err(e)) => Err(e.into()),
            Err(_) => Err(ServerTaskError::ReadTimeout),
        }
    }

    fn log_task_err(&self, e: ServerTaskError) {
        slog_info!(self.ctx.task_logger, "{}", e;
            "task_id" => LtUuid(&self.id),
            "start_at" => LtDateTime(&self.started),
            "server_addr" => self.ctx.local_addr,
            "client_addr" => self.ctx.peer_addr,
        );
    }

    fn log_task_ok(&self) {
        self.log_task_err(ServerTaskError::NoError)
    }
}
