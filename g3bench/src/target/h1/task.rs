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

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use futures_util::FutureExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::time::Instant;

use g3_http::client::HttpForwardRemoteResponse;
use g3_http::HttpBodyReader;
use g3_io_ext::{ArcLimitedReaderStats, ArcLimitedWriterStats, LimitedReader, LimitedWriter};

use super::{
    BenchHttpArgs, BenchTaskContext, HttpHistogramRecorder, HttpRuntimeStats, ProcArgs,
    SavedHttpForwardConnection,
};
use crate::target::BenchError;

pub(super) struct HttpTaskContext {
    args: Arc<BenchHttpArgs>,
    proc_args: Arc<ProcArgs>,
    saved_connection: Option<SavedHttpForwardConnection>,
    reuse_conn_count: u64,

    runtime_stats: Arc<HttpRuntimeStats>,
    histogram_recorder: Option<HttpHistogramRecorder>,

    req_header: Vec<u8>,
    req_header_fixed_len: usize,
}

impl HttpTaskContext {
    pub(super) fn new(
        args: &Arc<BenchHttpArgs>,
        proc_args: &Arc<ProcArgs>,
        runtime_stats: &Arc<HttpRuntimeStats>,
        histogram_recorder: Option<HttpHistogramRecorder>,
    ) -> anyhow::Result<Self> {
        let mut hdr_buf = Vec::with_capacity(1024);
        args.write_fixed_request_header(&mut hdr_buf)
            .map_err(|e| anyhow!("failed to generate request header: {}", e))?;

        let req_header_fixed_len = hdr_buf.len();

        Ok(HttpTaskContext {
            args: Arc::clone(args),
            proc_args: Arc::clone(proc_args),
            saved_connection: None,
            reuse_conn_count: 0,
            runtime_stats: Arc::clone(runtime_stats),
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

        if let Some(r) = &mut self.histogram_recorder {
            r.record_conn_reuse_count(self.reuse_conn_count);
        }
        self.reuse_conn_count = 0;

        self.runtime_stats.add_conn_attempt();
        let (r, w) = match tokio::time::timeout(
            self.args.connect_timeout,
            self.args.new_http_connection(&self.proc_args),
        )
        .await
        {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(anyhow!("timeout to get new connection")),
        };
        self.runtime_stats.add_conn_success();

        let r = LimitedReader::new(
            r,
            self.proc_args.tcp_sock_speed_limit.shift_millis,
            self.proc_args.tcp_sock_speed_limit.max_south,
            self.runtime_stats.clone() as ArcLimitedReaderStats,
        );
        let w = LimitedWriter::new(
            w,
            self.proc_args.tcp_sock_speed_limit.shift_millis,
            self.proc_args.tcp_sock_speed_limit.max_north,
            self.runtime_stats.clone() as ArcLimitedWriterStats,
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

        // send hdr
        ups_w
            .write_all(self.req_header.as_slice())
            .await
            .map_err(|e| anyhow!("failed to send request header: {e:?}"))?;
        let send_hdr_time = time_started.elapsed();
        if let Some(r) = &mut self.histogram_recorder {
            r.record_send_hdr_time(send_hdr_time);
        }

        // recv hdr
        let rsp = match tokio::time::timeout(
            self.args.timeout,
            HttpForwardRemoteResponse::parse(
                ups_r,
                &self.args.method,
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
        if let Some(r) = &mut self.histogram_recorder {
            r.record_recv_hdr_time(recv_hdr_time);
        }
        if let Some(ok_status) = self.args.ok_status {
            if rsp.code != ok_status.as_u16() {
                return Err(anyhow!(
                    "Got rsp code {} while {} is expected",
                    rsp.code,
                    ok_status.as_u16()
                ));
            }
        }

        // recv body
        if let Some(body_type) = rsp.body_type(&self.args.method) {
            let mut body_reader = HttpBodyReader::new(ups_r, body_type, 2048);
            let mut sink = tokio::io::sink();
            tokio::io::copy(&mut body_reader, &mut sink)
                .await
                .map_err(|e| anyhow!("failed to read response body: {e:?}"))?;
        }

        Ok(keep_alive & rsp.keep_alive())
    }
}

#[async_trait]
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
                if let Some(r) = &mut self.histogram_recorder {
                    r.record_total_time(total_time);
                }

                if keep_alive {
                    self.save_connection(connection);
                } else {
                    let runtime_stats = self.runtime_stats.clone();
                    tokio::spawn(async move {
                        // make sure the tls ticket will be reused
                        match tokio::time::timeout(
                            Duration::from_secs(4),
                            connection.writer.shutdown(),
                        )
                        .await
                        {
                            Ok(Ok(_)) => {}
                            Ok(Err(_e)) => runtime_stats.add_conn_close_fail(),
                            Err(_) => runtime_stats.add_conn_close_timeout(),
                        }
                    });
                }
                Ok(())
            }
            Err(e) => Err(BenchError::Task(e)),
        }
    }
}
