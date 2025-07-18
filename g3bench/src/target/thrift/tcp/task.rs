/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow};
use futures_util::FutureExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::Instant;

use g3_io_ext::{LimitedReadExt, LimitedReader, LimitedWriter};

use super::{ThriftConnection, ThriftHistogramRecorder, ThriftRuntimeStats, ThriftTcpArgs};
use crate::ProcArgs;
use crate::target::{BenchError, BenchTaskContext};

pub(super) struct ThriftTcpTaskContext {
    args: Arc<ThriftTcpArgs>,
    proc_args: Arc<ProcArgs>,

    send_buffer: Vec<u8>,
    send_header_size: usize,

    sequence_number: i32,
    saved_connection: Option<ThriftConnection>,
    reuse_conn_count: u64,

    runtime_stats: Arc<ThriftRuntimeStats>,
    histogram_recorder: ThriftHistogramRecorder,
}

impl Drop for ThriftTcpTaskContext {
    fn drop(&mut self) {
        self.histogram_recorder
            .record_conn_reuse_count(self.reuse_conn_count);
    }
}

impl ThriftTcpTaskContext {
    pub(super) fn new(
        args: &Arc<ThriftTcpArgs>,
        proc_args: &Arc<ProcArgs>,
        runtime_stats: &Arc<ThriftRuntimeStats>,
        histogram_recorder: ThriftHistogramRecorder,
    ) -> anyhow::Result<Self> {
        let send_buffer = Vec::new();
        let send_header_size = 0;

        Ok(ThriftTcpTaskContext {
            args: args.clone(),
            proc_args: proc_args.clone(),
            sequence_number: 0,
            send_buffer,
            send_header_size,
            saved_connection: None,
            reuse_conn_count: 0,
            runtime_stats: runtime_stats.clone(),
            histogram_recorder,
        })
    }

    async fn fetch_connection(&mut self) -> anyhow::Result<ThriftConnection> {
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
        let stream = match tokio::time::timeout(
            self.args.connect_timeout,
            self.args.new_tcp_connection(&self.proc_args),
        )
        .await
        {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(anyhow!("timeout to get new connection")),
        };
        self.runtime_stats.add_conn_success();

        let (r, w) = stream.into_split();
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
        Ok(ThriftConnection::new(r, w))
    }

    fn save_connection(&mut self, c: ThriftConnection) {
        self.saved_connection = Some(c);
    }

    fn update_sequence_number(&mut self) -> anyhow::Result<i32> {
        let mut id = self.sequence_number.wrapping_add(1);
        if id == 0 {
            id = 1;
        }
        self.sequence_number = id;

        self.send_buffer.resize(self.send_header_size, 0);
        self.args
            .global
            .request_builder
            .build(id, &mut self.send_buffer)?;

        Ok(id)
    }

    async fn run_with_connection(
        &mut self,
        _time_started: Instant,
        connection: &mut ThriftConnection,
        _seq_id: i32,
    ) -> anyhow::Result<()> {
        connection.writer.write_all(&self.send_buffer).await?;

        let mut recv_buffer = vec![0; 1024];
        let nr = connection.reader.read_all_once(&mut recv_buffer).await?;

        println!("{nr} bytes received");

        Ok(())
    }
}

impl BenchTaskContext for ThriftTcpTaskContext {
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
        // make the seq id rolling for the task instead of the connection
        let seq_id = self.update_sequence_number().map_err(BenchError::Fatal)?;

        let mut connection = self
            .fetch_connection()
            .await
            .context("connect to upstream failed")
            .map_err(BenchError::Fatal)?;

        match self
            .run_with_connection(time_started, &mut connection, seq_id)
            .await
        {
            Ok(_) => {
                let total_time = time_started.elapsed();
                self.histogram_recorder.record_total_time(total_time);

                if self.args.no_keepalive {
                    // make sure the tls ticket will be reused
                    match tokio::time::timeout(Duration::from_secs(4), connection.writer.shutdown())
                        .await
                    {
                        Ok(Ok(_)) => {}
                        Ok(Err(_e)) => {}
                        Err(_) => {}
                    }
                } else {
                    self.save_connection(connection);
                }
                Ok(())
            }
            Err(e) => Err(BenchError::Task(e)),
        }
    }
}
