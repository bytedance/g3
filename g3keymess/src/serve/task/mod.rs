/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use openssl::pkey::{PKey, Private};
use slog::{Logger, slog_info};
use tokio::io::AsyncRead;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, broadcast};
use tokio::time::Instant;
use uuid::Uuid;

use g3_daemon::server::ClientConnectionInfo;
use g3_histogram::HistogramRecorder;
use g3_slog_types::{LtDateTime, LtUuid};
use g3_std_ext::time::DurationExt;

use crate::config::server::KeyServerConfig;
use crate::protocol::{KeylessAction, KeylessErrorResponse, KeylessRequest, KeylessResponse};
use crate::serve::{
    KeyServerAliveTaskGuard, KeyServerDurationRecorder, KeyServerRequestStats, KeyServerStats,
    ServerReloadCommand, ServerTaskError,
};

mod multiplex;
mod simplex;

#[derive(Clone)]
pub(crate) struct RequestProcessContext {
    pub(crate) msg_id: u32,
    create_time: Instant,
    pub(crate) create_datetime: DateTime<Utc>,
    duration_recorder: Arc<HistogramRecorder<u64>>,
}

impl RequestProcessContext {
    fn new(msg_id: u32, duration_recorder: Arc<HistogramRecorder<u64>>) -> Self {
        RequestProcessContext {
            msg_id,
            create_time: Instant::now(),
            create_datetime: Utc::now(),
            duration_recorder,
        }
    }

    fn record_duration_stats(&self) {
        let _ = self
            .duration_recorder
            .record(self.duration().as_nanos_u64());
    }

    pub(crate) fn duration(&self) -> Duration {
        self.create_time.elapsed()
    }
}

pub(crate) struct WrappedKeylessResponse {
    inner: KeylessResponse,
    ctx: RequestProcessContext,
}

impl WrappedKeylessResponse {
    pub(crate) fn new(inner: KeylessResponse, ctx: RequestProcessContext) -> Self {
        WrappedKeylessResponse { inner, ctx }
    }
}

pub(crate) struct WrappedKeylessRequest {
    pub(crate) inner: KeylessRequest,
    pub(crate) stats: Arc<KeyServerRequestStats>,
    ctx: RequestProcessContext,
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
        let ctx = RequestProcessContext::new(req.id, duration_recorder);
        WrappedKeylessRequest {
            inner: req,
            stats,
            ctx,
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
        WrappedKeylessResponse::new(rsp, self.ctx.clone())
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
    pub(crate) cc_info: ClientConnectionInfo,
    pub(crate) task_logger: Option<Logger>,
    pub(crate) request_logger: Option<Logger>,
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
    _alive_guard: KeyServerAliveTaskGuard,
}

impl KeylessTask {
    pub(crate) fn new(ctx: KeylessTaskContext) -> Self {
        let alive_guard = ctx.server_stats.add_task();
        let started = Utc::now();
        KeylessTask {
            id: g3_daemon::server::task::generate_uuid(&started),
            ctx,
            started,
            buf: Vec::with_capacity(crate::protocol::MESSAGE_PADDED_LENGTH + 2),
            #[cfg(feature = "openssl-async-job")]
            allow_openssl_async_job: false,
            allow_dispatch: false,
            _alive_guard: alive_guard,
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
        if let Some(logger) = &self.ctx.task_logger {
            slog_info!(logger, "{}", e;
                "task_id" => LtUuid(&self.id),
                "start_at" => LtDateTime(&self.started),
                "server_addr" => self.ctx.cc_info.server_addr(),
                "client_addr" => self.ctx.cc_info.client_addr(),
            );
        }
    }

    fn log_task_ok(&self) {
        self.log_task_err(ServerTaskError::NoError)
    }
}
