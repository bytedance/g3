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

use anyhow::anyhow;
use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::Instant;

use g3_io_ext::LimitedStream;

use super::{BenchSslArgs, BenchTaskContext, ProcArgs, SslHistogramRecorder, SslRuntimeStats};
use crate::target::BenchError;

pub(super) struct SslTaskContext {
    args: Arc<BenchSslArgs>,
    proc_args: Arc<ProcArgs>,

    runtime_stats: Arc<SslRuntimeStats>,
    histogram_recorder: Option<SslHistogramRecorder>,
}

impl SslTaskContext {
    pub(super) fn new(
        args: &Arc<BenchSslArgs>,
        proc_args: &Arc<ProcArgs>,
        runtime_stats: &Arc<SslRuntimeStats>,
        histogram_recorder: Option<SslHistogramRecorder>,
    ) -> anyhow::Result<Self> {
        Ok(SslTaskContext {
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
        Ok(LimitedStream::new(
            stream,
            speed_limit.shift_millis,
            speed_limit.max_south,
            speed_limit.max_north,
            self.runtime_stats.clone(),
        ))
    }
}

#[async_trait]
impl BenchTaskContext for SslTaskContext {
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
                if let Some(r) = &mut self.histogram_recorder {
                    r.record_total_time(total_time);
                }

                let runtime_stats = self.runtime_stats.clone();
                // make sure the tls ticket will be reused
                match tokio::time::timeout(Duration::from_secs(4), tls_stream.shutdown()).await {
                    Ok(Ok(_)) => {}
                    Ok(Err(_e)) => runtime_stats.add_conn_close_fail(),
                    Err(_) => runtime_stats.add_conn_close_timeout(),
                }

                Ok(())
            }
            Ok(Err(e)) => Err(BenchError::Task(e)),
            Err(_) => Err(BenchError::Task(anyhow!("tls handshake timeout"))),
        }
    }
}
