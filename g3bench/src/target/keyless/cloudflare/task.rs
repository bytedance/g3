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

use super::{
    BenchTaskContext, KeylessCloudflareArgs, KeylessConnectionPool, KeylessHistogramRecorder,
    KeylessRequest, KeylessRuntimeStats, SendHandle,
};
use crate::opts::ProcArgs;
use crate::target::keyless::cloudflare::message::KeylessRequestBuilder;

pub(super) struct KeylessCloudflareTaskContext {
    args: Arc<KeylessCloudflareArgs>,
    proc_args: Arc<ProcArgs>,

    pool: Option<Arc<KeylessConnectionPool>>,
    save: Option<SendHandle>,

    reuse_conn_count: u64,
    request_message: KeylessRequest,

    runtime_stats: Arc<KeylessRuntimeStats>,
    histogram_recorder: Option<KeylessHistogramRecorder>,
}

impl KeylessCloudflareTaskContext {
    pub(super) fn new(
        args: &Arc<KeylessCloudflareArgs>,
        proc_args: &Arc<ProcArgs>,
        runtime_stats: &Arc<KeylessRuntimeStats>,
        histogram_recorder: Option<KeylessHistogramRecorder>,
        pool: Option<Arc<KeylessConnectionPool>>,
    ) -> anyhow::Result<Self> {
        let pkey_digest = args.global.get_public_key_digest()?;
        let request_builder = KeylessRequestBuilder::new(pkey_digest, args.global.action)?;
        let request_message = request_builder.build(&args.global.payload)?;
        Ok(KeylessCloudflareTaskContext {
            args: Arc::clone(args),
            proc_args: Arc::clone(proc_args),
            pool,
            save: None,
            reuse_conn_count: 0,
            request_message,
            runtime_stats: Arc::clone(runtime_stats),
            histogram_recorder,
        })
    }

    async fn fetch_handle(&mut self) -> anyhow::Result<SendHandle> {
        if let Some(pool) = &self.pool {
            return pool.fetch_handle().await;
        }

        if let Some(handle) = &self.save {
            if !handle.is_closed() {
                return Ok(handle.clone());
            }
            self.save = None;
        }

        if self.reuse_conn_count > 0 {
            if let Some(r) = &mut self.histogram_recorder {
                r.record_conn_reuse_count(self.reuse_conn_count);
            }
            self.reuse_conn_count = 0;
        }

        self.runtime_stats.add_conn_attempt();
        let handle = match tokio::time::timeout(
            self.args.connect_timeout,
            self.args.new_keyless_connection(&self.proc_args),
        )
        .await
        {
            Ok(Ok(h)) => h,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(anyhow!("timeout to get new connection")),
        };
        self.runtime_stats.add_conn_success();

        self.save = Some(handle.clone());
        Ok(handle)
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

    async fn run(&mut self, task_id: usize, time_started: Instant) -> anyhow::Result<()> {
        let handle = self.fetch_handle().await?;

        match handle.send_request(self.request_message.clone()).await {
            Some(rsp) => {
                let total_time = time_started.elapsed();
                if let Some(r) = &mut self.histogram_recorder {
                    r.record_total_time(total_time);
                }
                self.args.global.dump_result(task_id, rsp.into_vec());
                Ok(())
            }
            None => {
                self.save = None;
                match handle.fetch_error() {
                    Some(e) => Err(anyhow!(e)),
                    None => Err(anyhow!("we get no response but no error reported")),
                }
            }
        }
    }
}
