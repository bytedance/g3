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
use openssl::pkey::{PKey, Private};
use slog::{slog_info, Logger};
use tokio::io::AsyncRead;
use tokio::sync::{broadcast, OwnedSemaphorePermit, Semaphore};
use tokio::time::Instant;
use uuid::Uuid;

use g3_histogram::HistogramRecorder;
use g3_slog_types::{LtDateTime, LtUuid};

use crate::config::server::KeyServerConfig;
use crate::protocol::{KeylessAction, KeylessErrorResponse, KeylessRequest, KeylessResponse};
use crate::serve::{
    KeyServerDurationRecorder, KeyServerRequestStats, KeyServerStats, ServerReloadCommand,
    ServerTaskError,
};

mod multiplex;
mod simplex;

pub(crate) struct WrappedKeylessResponse {
    pub(crate) inner: KeylessResponse,
    create_time: Instant,
    duration_recorder: Arc<HistogramRecorder<u64>>,
}

impl WrappedKeylessResponse {
    pub(crate) fn new(
        inner: KeylessResponse,
        create_time: Instant,
        duration_recorder: Arc<HistogramRecorder<u64>>,
    ) -> Self {
        WrappedKeylessResponse {
            inner,
            create_time,
            duration_recorder,
        }
    }
}

pub(crate) struct WrappedKeylessRequest {
    pub(crate) inner: KeylessRequest,
    pub(crate) stats: Arc<KeyServerRequestStats>,
    duration_recorder: Arc<HistogramRecorder<u64>>,
    create_time: Instant,
    err_rsp: Option<KeylessErrorResponse>,
    server_sem_permit: Option<OwnedSemaphorePermit>,
}

impl WrappedKeylessRequest {
    fn new(
        mut req: KeylessRequest,
        server_stats: &Arc<KeyServerStats>,
        duration_recorder: &KeyServerDurationRecorder,
    ) -> Self {
        let err_rsp = req.verify_opcode().err();
        let (stats, duration_recorder) = match req.action {
            KeylessAction::Ping => (
                server_stats.ping_pong.clone(),
                duration_recorder.ping_pong.clone(),
            ),
            KeylessAction::RsaDecrypt(_) => (
                server_stats.rsa_decrypt.clone(),
                duration_recorder.rsa_decrypt.clone(),
            ),
            KeylessAction::RsaSign(_) => (
                server_stats.rsa_sign.clone(),
                duration_recorder.rsa_sign.clone(),
            ),
            KeylessAction::RsaPssSign(_) => (
                server_stats.rsa_pss_sign.clone(),
                duration_recorder.rsa_pss_sign.clone(),
            ),
            KeylessAction::EcdsaSign(_) => (
                server_stats.ecdsa_sign.clone(),
                duration_recorder.ecdsa_sign.clone(),
            ),
            KeylessAction::Ed25519Sign => (
                server_stats.ed25519_sign.clone(),
                duration_recorder.ed25519_sign.clone(),
            ),
            KeylessAction::NotSet => (server_stats.noop.clone(), duration_recorder.noop.clone()),
        };
        stats.add_total();
        stats.inc_alive();
        WrappedKeylessRequest {
            inner: req,
            stats,
            duration_recorder,
            create_time: Instant::now(),
            err_rsp,
            server_sem_permit: None,
        }
    }

    fn take_err_rsp(&mut self) -> Option<KeylessErrorResponse> {
        self.err_rsp.take()
    }

    pub(crate) fn process_by_openssl(&self, key: &PKey<Private>) -> KeylessResponse {
        match self.inner.process(key) {
            Ok(d) => {
                self.stats.add_passed();
                KeylessResponse::Data(d)
            }
            Err(e) => {
                self.stats.add_by_error_code(e.error_code());
                KeylessResponse::Error(e)
            }
        }
    }

    pub(crate) fn build_response(&self, rsp: KeylessResponse) -> WrappedKeylessResponse {
        WrappedKeylessResponse::new(rsp, self.create_time, self.duration_recorder.clone())
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
    pub(crate) duration_recorder: KeyServerDurationRecorder,
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
    #[cfg(feature = "openssl-async-job")]
    allow_openssl_async_job: bool,
    allow_dispatch: bool,
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
            #[cfg(feature = "openssl-async-job")]
            allow_openssl_async_job: false,
            allow_dispatch: false,
        }
    }

    pub(crate) fn set_allow_dispatch(&mut self) {
        self.allow_dispatch = true;
    }

    #[cfg(feature = "openssl-async-job")]
    pub(crate) fn set_allow_openssl_async_job(&mut self) {
        self.allow_openssl_async_job = true;
    }

    async fn timed_read_request<R>(
        &mut self,
        reader: &mut R,
        msg_count: usize,
    ) -> Result<WrappedKeylessRequest, ServerTaskError>
    where
        R: AsyncRead + Unpin,
    {
        match tokio::time::timeout(
            self.ctx.server_config.request_read_timeout,
            KeylessRequest::read(reader, &mut self.buf, msg_count),
        )
        .await
        {
            Ok(Ok(req)) => Ok(WrappedKeylessRequest::new(
                req,
                &self.ctx.server_stats,
                &self.ctx.duration_recorder,
            )),
            Ok(Err(e)) => Err(e.into()),
            Err(_) => Err(ServerTaskError::ReadTimeout),
        }
    }

    fn log_task_err(&self, e: ServerTaskError) {
        if e.ignore_log() {
            return;
        }
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
