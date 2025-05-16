/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::Instant;

use g3_io_ext::LimitedStream;

use super::{BenchOpensslArgs, BenchTaskContext, ProcArgs, SslHistogramRecorder, SslRuntimeStats};
use crate::target::BenchError;

pub(super) struct OpensslTaskContext {
    args: Arc<BenchOpensslArgs>,
    proc_args: Arc<ProcArgs>,

    runtime_stats: Arc<SslRuntimeStats>,
    histogram_recorder: SslHistogramRecorder,
}

impl OpensslTaskContext {
    pub(super) fn new(
        args: &Arc<BenchOpensslArgs>,
        proc_args: &Arc<ProcArgs>,
        runtime_stats: &Arc<SslRuntimeStats>,
        histogram_recorder: SslHistogramRecorder,
    ) -> anyhow::Result<Self> {
        Ok(OpensslTaskContext {
            args: Arc::clone(args),
            proc_args: Arc::clone(proc_args),
            runtime_stats: Arc::clone(runtime_stats),
            histogram_recorder,
        })
    }

    async fn connect(&self) -> anyhow::Result<LimitedStream<TcpStream>> {
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

        let speed_limit = &self.proc_args.tcp_sock_speed_limit;
        Ok(LimitedStream::local_limited(
            stream,
            speed_limit.shift_millis,
            speed_limit.max_south,
            speed_limit.max_north,
            self.runtime_stats.clone(),
        ))
    }
}

impl BenchTaskContext for OpensslTaskContext {
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
        let tcp_stream = self.connect().await.map_err(BenchError::Fatal)?;

        let tls_client = self.args.tls.client.as_ref().unwrap();
        match tokio::time::timeout(
            self.args.timeout,
            self.args.tls_connect_to_target(tls_client, tcp_stream),
        )
        .await
        {
            Ok(Ok(mut tls_stream)) => {
                let total_time = time_started.elapsed();
                self.histogram_recorder.record_total_time(total_time);

                self.runtime_stats.session.add_total();
                if tls_stream.ssl().session_reused() {
                    self.runtime_stats.session.add_reused();
                }

                // make sure the tls ticket will be reused
                match tokio::time::timeout(Duration::from_secs(4), tls_stream.shutdown()).await {
                    Ok(Ok(_)) => {}
                    Ok(Err(_e)) => self.runtime_stats.add_conn_close_fail(),
                    Err(_) => self.runtime_stats.add_conn_close_timeout(),
                }

                Ok(())
            }
            Ok(Err(e)) => Err(BenchError::Task(e)),
            Err(_) => Err(BenchError::Task(anyhow!("tls handshake timeout"))),
        }
    }
}
