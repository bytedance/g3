/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::IoSlice;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow};
use futures_util::FutureExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::time::Instant;

use g3_http::HttpBodyReader;
use g3_http::client::HttpForwardRemoteResponse;
use g3_io_ext::{LimitedReader, LimitedWriteExt, LimitedWriter};

use super::{BenchHttpArgs, BenchTaskContext, HttpHistogramRecorder, HttpRuntimeStats, ProcArgs};
use crate::module::http::SavedHttpForwardConnection;
use crate::target::BenchError;

pub(super) struct HttpTaskContext {
    args: Arc<BenchHttpArgs>,
    proc_args: Arc<ProcArgs>,
    saved_connection: Option<SavedHttpForwardConnection>,
    reuse_conn_count: u64,

    runtime_stats: Arc<HttpRuntimeStats>,
    histogram_recorder: HttpHistogramRecorder,

    req_header: Vec<u8>,
    req_header_fixed_len: usize,
}

impl HttpTaskContext {
    pub(super) fn new(
        args: Arc<BenchHttpArgs>,
        proc_args: Arc<ProcArgs>,
        runtime_stats: Arc<HttpRuntimeStats>,
        histogram_recorder: HttpHistogramRecorder,
    ) -> anyhow::Result<Self> {
        let mut hdr_buf = Vec::with_capacity(1024);
        args.write_fixed_request_header(&mut hdr_buf)
            .map_err(|e| anyhow!("failed to generate request header: {}", e))?;

        let req_header_fixed_len = hdr_buf.len();

        Ok(HttpTaskContext {
            args: args.clone(),
            proc_args: proc_args.clone(),
            saved_connection: None,
            reuse_conn_count: 0,
            runtime_stats: runtime_stats.clone(),
            histogram_recorder,
            req_header: hdr_buf,
            req_header_fixed_len,
        })
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
        self.runtime_stats.add_conn_success();

        let r = LimitedReader::local_limited(
            r,
            self.proc_args.tcp_sock_speed_limit.shift_millis,
            self.proc_args.tcp_sock_speed_limit.max_south,
            self.runtime_stats.clone(),
        );
        let w = LimitedWriter::local_limited(
            w,
            self.proc_args.tcp_sock_speed_limit.shift_millis,
            self.proc_args.tcp_sock_speed_limit.max_north,
            self.runtime_stats.clone(),
        );
        Ok(SavedHttpForwardConnection::new(BufReader::new(r), w))
    }

    fn save_connection(&mut self, c: SavedHttpForwardConnection) {
        self.saved_connection = Some(c);
    }

    fn reset_request_header(&mut self) {
        // reset request header
        self.req_header.truncate(self.req_header_fixed_len);
        // TODO generate dynamic header
        self.req_header.extend_from_slice(b"\r\n");
    }

    async fn run_with_connection(
        &mut self,
        time_started: Instant,
        connection: &mut SavedHttpForwardConnection,
    ) -> anyhow::Result<bool> {
        let keep_alive = !self.args.no_keepalive;
        let ups_r = &mut connection.reader;
        let ups_w = &mut connection.writer;

        if let Some(data) = self.args.common.payload() {
            ups_w
                .write_all_vectored([IoSlice::new(&self.req_header), IoSlice::new(data)])
                .await
                .map_err(|e| anyhow!("failed to send request header and body in a batch: {e:?}"))?;
            ups_w
                .flush()
                .await
                .map_err(|e| anyhow!("write flush failed: {e:?}"))?;
            self.histogram_recorder
                .record_send_all_time(time_started.elapsed());
        } else {
            // send hdr
            ups_w
                .write_all_flush(self.req_header.as_slice())
                .await
                .map_err(|e| anyhow!("failed to send request header: {e:?}"))?;
            let send_hdr_time = time_started.elapsed();
            self.histogram_recorder.record_send_hdr_time(send_hdr_time);
            self.histogram_recorder.record_send_all_time(send_hdr_time);
        }

        // recv hdr
        let rsp = match tokio::time::timeout(
            self.args.common.timeout,
            HttpForwardRemoteResponse::parse(
                ups_r,
                &self.args.common.method,
                keep_alive,
                self.args.max_header_size,
            ),
        )
        .await
        {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => return Err(anyhow!("failed to read response: {e}")),
            Err(_) => return Err(anyhow!("timeout to read response")),
        };

        let recv_hdr_time = time_started.elapsed();
        self.histogram_recorder.record_recv_hdr_time(recv_hdr_time);
        if let Some(ok_status) = self.args.common.ok_status
            && rsp.code != ok_status.as_u16()
        {
            return Err(anyhow!(
                "Got rsp code {} while {} is expected",
                rsp.code,
                ok_status.as_u16()
            ));
        }

        // recv body
        if let Some(body_type) = rsp.body_type(&self.args.common.method) {
            let mut body_reader = HttpBodyReader::new(ups_r, body_type, 2048);
            let mut sink = tokio::io::sink();
            tokio::io::copy(&mut body_reader, &mut sink)
                .await
                .map_err(|e| anyhow!("failed to read response body: {e:?}"))?;
        }

        Ok(keep_alive & rsp.keep_alive())
    }
}

impl BenchTaskContext for HttpTaskContext {
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
        self.reset_request_header();

        let mut connection = self
            .fetch_connection()
            .await
            .context("connect to upstream failed")
            .map_err(BenchError::Fatal)?;

        match self
            .run_with_connection(time_started, &mut connection)
            .await
        {
            Ok(keep_alive) => {
                let total_time = time_started.elapsed();
                self.histogram_recorder.record_total_time(total_time);

                if keep_alive {
                    self.save_connection(connection);
                } else {
                    // make sure the tls ticket will be reused
                    match tokio::time::timeout(Duration::from_secs(4), connection.writer.shutdown())
                        .await
                    {
                        Ok(Ok(_)) => {}
                        Ok(Err(_e)) => self.runtime_stats.add_conn_close_fail(),
                        Err(_) => self.runtime_stats.add_conn_close_timeout(),
                    }
                }
                Ok(())
            }
            Err(e) => Err(BenchError::Task(e)),
        }
    }
}
