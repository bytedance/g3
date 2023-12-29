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
use tokio::time::Instant;

use super::{
    BenchTaskContext, KeylessCloudflareArgs, KeylessConnectionPool, KeylessHistogramRecorder,
    KeylessRequest, KeylessRequestBuilder, KeylessResponse, KeylessRuntimeStats, MultiplexTransfer,
    SimplexTransfer,
};
use crate::opts::ProcArgs;
use crate::target::BenchError;

pub(super) struct KeylessCloudflareTaskContext {
    args: Arc<KeylessCloudflareArgs>,
    proc_args: Arc<ProcArgs>,

    pool: Option<Arc<KeylessConnectionPool>>,
    multiplex: Option<Arc<MultiplexTransfer>>,
    simplex: Option<SimplexTransfer>,

    reuse_conn_count: u64,
    request_message: KeylessRequest,

    runtime_stats: Arc<KeylessRuntimeStats>,
    histogram_recorder: KeylessHistogramRecorder,
}

impl Drop for KeylessCloudflareTaskContext {
    fn drop(&mut self) {
        self.histogram_recorder
            .record_conn_reuse_count(self.reuse_conn_count);
    }
}

impl KeylessCloudflareTaskContext {
    pub(super) fn new(
        args: &Arc<KeylessCloudflareArgs>,
        proc_args: &Arc<ProcArgs>,
        runtime_stats: &Arc<KeylessRuntimeStats>,
        histogram_recorder: KeylessHistogramRecorder,
        pool: Option<Arc<KeylessConnectionPool>>,
    ) -> anyhow::Result<Self> {
        let request_builder =
            KeylessRequestBuilder::new(args.global.subject_key_id(), args.global.action)?;
        let request_message = request_builder.build(&args.global.payload)?;
        Ok(KeylessCloudflareTaskContext {
            args: Arc::clone(args),
            proc_args: Arc::clone(proc_args),
            pool,
            multiplex: None,
            simplex: None,
            reuse_conn_count: 0,
            request_message,
            runtime_stats: Arc::clone(runtime_stats),
            histogram_recorder,
        })
    }

    async fn fetch_multiplex_handle(&mut self) -> anyhow::Result<Arc<MultiplexTransfer>> {
        if let Some(pool) = &self.pool {
            return pool.fetch_handle().await;
        }

        if let Some(handle) = &self.multiplex {
            if !handle.is_closed() {
                self.reuse_conn_count += 1;
                return Ok(handle.clone());
            }
            self.multiplex = None;
        }

        if self.reuse_conn_count > 0 {
            self.histogram_recorder
                .record_conn_reuse_count(self.reuse_conn_count);
            self.reuse_conn_count = 0;
        }

        self.runtime_stats.add_conn_attempt();
        let handle = match tokio::time::timeout(
            self.args.connect_timeout,
            self.args.new_multiplex_keyless_connection(&self.proc_args),
        )
        .await
        {
            Ok(Ok(h)) => Arc::new(h),
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(anyhow!("timeout to get new connection")),
        };
        self.runtime_stats.add_conn_success();

        self.multiplex = Some(handle.clone());
        Ok(handle)
    }

    async fn fetch_simplex_connection(&mut self) -> anyhow::Result<SimplexTransfer> {
        if let Some(mut c) = self.simplex.take() {
            if !c.is_closed() {
                self.reuse_conn_count += 1;
                return Ok(c);
            }
        }

        if self.reuse_conn_count > 0 {
            self.histogram_recorder
                .record_conn_reuse_count(self.reuse_conn_count);
            self.reuse_conn_count = 0;
        }

        self.runtime_stats.add_conn_attempt();
        match tokio::time::timeout(
            self.args.connect_timeout,
            self.args.new_simplex_keyless_connection(&self.proc_args),
        )
        .await
        {
            Ok(Ok(c)) => {
                self.runtime_stats.add_conn_success();
                Ok(c)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow!("timeout to get new connection")),
        }
    }

    async fn do_run_multiplex(
        &self,
        handle: &MultiplexTransfer,
    ) -> anyhow::Result<KeylessResponse> {
        match tokio::time::timeout(
            self.args.timeout,
            handle.send_request(self.request_message.clone()),
        )
        .await
        {
            Ok(Ok(rsp)) => Ok(rsp),
            Ok(Err(id)) => match handle.fetch_error() {
                Some(e) => Err(anyhow!("{}/{id} error: {e}", handle.local_addr())),
                None => Err(anyhow!(
                    "{}/{id}: we get no response but no error reported",
                    handle.local_addr()
                )),
            },
            Err(_) => Err(anyhow!("{}: request timed out", handle.local_addr())),
        }
    }

    async fn do_run_simplex(
        &mut self,
        connection: &mut SimplexTransfer,
    ) -> anyhow::Result<KeylessResponse> {
        match tokio::time::timeout(
            self.args.timeout,
            connection.send_request(&mut self.request_message),
        )
        .await
        {
            Ok(Ok(rsp)) => Ok(rsp),
            Ok(Err(e)) => Err(anyhow!("{} error: {e}", connection.local_addr())),
            Err(_) => Err(anyhow!("{}: request timed out", connection.local_addr())),
        }
    }
}

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

    async fn run(&mut self, task_id: usize, time_started: Instant) -> Result<(), BenchError> {
        if self.args.no_multiplex {
            let mut connection = self
                .fetch_simplex_connection()
                .await
                .map_err(BenchError::Fatal)?;

            match self.do_run_simplex(&mut connection).await {
                Ok(rsp) => {
                    let total_time = time_started.elapsed();
                    self.simplex = Some(connection);
                    self.histogram_recorder.record_total_time(total_time);
                    self.args
                        .global
                        .check_result(task_id, rsp.into_vec())
                        .map_err(BenchError::Task)
                }
                Err(e) => Err(BenchError::Task(e)),
            }
        } else {
            let handle = self
                .fetch_multiplex_handle()
                .await
                .map_err(BenchError::Fatal)?;

            match self.do_run_multiplex(&handle).await {
                Ok(rsp) => {
                    let total_time = time_started.elapsed();
                    self.histogram_recorder.record_total_time(total_time);
                    self.args
                        .global
                        .check_result(task_id, rsp.into_vec())
                        .map_err(BenchError::Task)
                }
                Err(e) => {
                    self.multiplex = None;
                    Err(BenchError::Task(e))
                }
            }
        }
    }
}
