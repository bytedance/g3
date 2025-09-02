/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::{Context, anyhow};
use futures_util::FutureExt;
use http::Method;
use tokio::io::{AsyncBufRead, AsyncReadExt, AsyncWrite, BufReader};
use tokio::time::Instant;

use g3_http::client::HttpTransparentResponse;
use g3_io_ext::{LimitedReader, LimitedWriteExt, LimitedWriter};
use g3_types::net::HttpUpgradeToken;

use super::H1WebsocketArgs;
use crate::ProcArgs;
use crate::module::http::{HttpHistogramRecorder, HttpRuntimeStats, SavedHttpForwardConnection};
use crate::target::{BenchError, BenchTaskContext};

pub(super) struct H1WebsocketTaskContext {
    args: Arc<H1WebsocketArgs>,
    proc_args: Arc<ProcArgs>,
    saved_connection: Option<SavedHttpForwardConnection>,
    reuse_conn_count: u64,

    runtime_stats: Arc<HttpRuntimeStats>,
    histogram_recorder: HttpHistogramRecorder,
}

impl H1WebsocketTaskContext {
    pub(super) fn new(
        args: Arc<H1WebsocketArgs>,
        proc_args: Arc<ProcArgs>,
        runtime_stats: Arc<HttpRuntimeStats>,
        histogram_recorder: HttpHistogramRecorder,
    ) -> Self {
        H1WebsocketTaskContext {
            args,
            proc_args,
            saved_connection: None,
            reuse_conn_count: 0,
            runtime_stats,
            histogram_recorder,
        }
    }

    async fn upgrade<R, W>(&self, writer: &mut W, reader: &mut R) -> anyhow::Result<()>
    where
        R: AsyncBufRead + Send + Sync + Unpin,
        W: AsyncWrite + Send + Sync + Unpin,
    {
        let mut buf = Vec::with_capacity(512);
        let key = self
            .args
            .build_upgrade_request(&mut buf)
            .context("failed to build upgrade request")?;

        writer
            .write_all_flush(&buf)
            .await
            .map_err(|e| anyhow!("failed to write upgrade request: {e}"))?;

        let (rsp, _) = HttpTransparentResponse::parse(reader, &Method::GET, true, 1024).await?;
        if rsp.code != 101 {
            return Err(anyhow!(
                "upgrade failed, code: {}, reason: {}",
                rsp.code,
                rsp.reason
            ));
        }
        if !matches!(rsp.upgrade, Some(HttpUpgradeToken::Websocket)) {
            return Err(anyhow!(
                "no valid 'Upgrade' header found or 'Connection' contains no 'Upgrade'"
            ));
        }

        self.args
            .common
            .verify_upgrade_response_headers(key, rsp.end_to_end_headers.into())?;
        Ok(())
    }

    async fn fetch_connection(&mut self) -> anyhow::Result<SavedHttpForwardConnection> {
        if let Some(mut c) = self.saved_connection.take() {
            let mut buf = [0u8; 4];
            if c.reader.read(&mut buf).now_or_never().is_none() {
                // no eof, reuse the old connection
                self.reuse_conn_count += 1;
                return Ok(c);
            }
        }

        self.histogram_recorder
            .record_conn_reuse_count(self.reuse_conn_count);
        self.reuse_conn_count = 0;

        self.runtime_stats.add_conn_attempt();
        let (r, w) = match tokio::time::timeout(
            self.args.common.connect_timeout,
            self.args.connect.new_http_connection(
                &self.args.common.target,
                &self.runtime_stats,
                &self.proc_args,
            ),
        )
        .await
        {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(anyhow!("timeout to get new connection")),
        };

        let r = LimitedReader::local_limited(
            r,
            self.proc_args.tcp_sock_speed_limit.shift_millis,
            self.proc_args.tcp_sock_speed_limit.max_south,
            self.runtime_stats.clone(),
        );
        let mut w = LimitedWriter::local_limited(
            w,
            self.proc_args.tcp_sock_speed_limit.shift_millis,
            self.proc_args.tcp_sock_speed_limit.max_north,
            self.runtime_stats.clone(),
        );

        let mut r = BufReader::new(r);
        tokio::time::timeout(
            self.args.common.upgrade_timeout,
            self.upgrade(&mut w, &mut r),
        )
        .await
        .map_err(|_| anyhow!("websocket upgrade timed out"))??;

        self.runtime_stats.add_conn_success();
        Ok(SavedHttpForwardConnection::new(r, w))
    }

    fn save_connection(&mut self, c: SavedHttpForwardConnection) {
        self.saved_connection = Some(c);
    }

    async fn run_with_connection(
        &mut self,
        time_started: Instant,
        connection: &mut SavedHttpForwardConnection,
    ) -> anyhow::Result<()> {
        todo!()
    }
}

impl BenchTaskContext for H1WebsocketTaskContext {
    fn mark_task_start(&self) {
        self.runtime_stats.add_task_total();
        self.runtime_stats.inc_task_alive();
    }

    fn mark_task_passed(&self) {
        self.runtime_stats.add_task_passed();
        self.runtime_stats.dec_task_alive();
    }

    fn mark_task_failed(&self) {
        self.runtime_stats.add_task_failed();
        self.runtime_stats.dec_task_alive();
    }

    async fn run(&mut self, _task_id: usize, time_started: Instant) -> Result<(), BenchError> {
        let mut connection = self
            .fetch_connection()
            .await
            .context("connect to upstream failed")
            .map_err(BenchError::Fatal)?;

        match self
            .run_with_connection(time_started, &mut connection)
            .await
        {
            Ok(_) => {
                let total_time = time_started.elapsed();
                self.histogram_recorder.record_total_time(total_time);
                self.save_connection(connection);
                Ok(())
            }
            Err(e) => Err(BenchError::Task(e)),
        }
    }
}
