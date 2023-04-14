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

use anyhow::anyhow;
use async_trait::async_trait;
use tokio::time::Instant;

use super::{BenchTaskContext, BoxKeylessConnection, KeylessCloudflareArgs};
use crate::opts::ProcArgs;
use crate::target::ssl::{SslHistogramRecorder, SslRuntimeStats};

pub(super) struct KeylessCloudflareTaskContext {
    args: Arc<KeylessCloudflareArgs>,
    proc_args: Arc<ProcArgs>,

    runtime_stats: Arc<SslRuntimeStats>,
    histogram_recorder: Option<SslHistogramRecorder>,
}

impl KeylessCloudflareTaskContext {
    pub(super) fn new(
        args: &Arc<KeylessCloudflareArgs>,
        proc_args: &Arc<ProcArgs>,
        runtime_stats: &Arc<SslRuntimeStats>,
        histogram_recorder: Option<SslHistogramRecorder>,
    ) -> anyhow::Result<Self> {
        Ok(KeylessCloudflareTaskContext {
            args: Arc::clone(args),
            proc_args: Arc::clone(proc_args),
            runtime_stats: Arc::clone(runtime_stats),
            histogram_recorder,
        })
    }

    async fn connect(&self) -> anyhow::Result<BoxKeylessConnection> {
        self.runtime_stats.add_conn_attempt();
        let conn = match tokio::time::timeout(
            self.args.connect_timeout,
            self.args.new_keyless_connection(&self.proc_args),
        )
        .await
        {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(anyhow!("timeout to get new connection")),
        };
        self.runtime_stats.add_conn_success();
        Ok(conn)
    }
}

#[async_trait]
impl BenchTaskContext for KeylessCloudflareTaskContext {
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

    async fn run(&mut self, _task_id: usize, time_started: Instant) -> anyhow::Result<()> {
        let tcp_stream = self.connect().await?;

        // TODO
        todo!()
    }
}
