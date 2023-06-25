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

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use tokio::time::Instant;
use trust_dns_client::client::{AsyncClient, ClientHandle};
use trust_dns_proto::op::ResponseCode;

use super::{
    BenchDnsArgs, BenchTaskContext, DnsHistogramRecorder, DnsRequestPickState, DnsRuntimeStats,
};
use crate::target::dns::DnsRequest;
use crate::target::BenchError;

#[derive(Default)]
struct LocalRequestPicker {
    id: usize,
}

impl LocalRequestPicker {
    fn set_id(&self, v: usize) {
        unsafe {
            let p = &self.id as *const usize as *mut usize;
            *p = v;
        }
    }
}

impl DnsRequestPickState for LocalRequestPicker {
    fn pick_next(&self, max: usize) -> usize {
        let next = self.id;
        if self.id >= max {
            self.set_id(0);
        } else {
            self.set_id(self.id + 1);
        }
        next
    }
}

pub(super) struct DnsTaskContext {
    args: Arc<BenchDnsArgs>,

    client: Option<AsyncClient>,

    runtime_stats: Arc<DnsRuntimeStats>,
    histogram_recorder: Option<DnsHistogramRecorder>,

    local_picker: LocalRequestPicker,
}

impl DnsTaskContext {
    pub(super) fn new(
        args: &Arc<BenchDnsArgs>,
        runtime_stats: &Arc<DnsRuntimeStats>,
        histogram_recorder: Option<DnsHistogramRecorder>,
    ) -> anyhow::Result<Self> {
        Ok(DnsTaskContext {
            args: Arc::clone(args),
            client: None,
            runtime_stats: Arc::clone(runtime_stats),
            histogram_recorder,
            local_picker: LocalRequestPicker::default(),
        })
    }

    async fn fetch_client(&mut self) -> anyhow::Result<AsyncClient> {
        if let Some(client) = &self.client {
            return Ok(client.clone());
        }

        self.runtime_stats.add_conn_attempt();
        let client = self.args.new_dns_client().await?;
        self.runtime_stats.add_conn_success();
        self.client = Some(client.clone());
        Ok(client)
    }

    fn drop_client(&mut self) {
        self.client = None;
    }

    async fn run_with_client(
        &self,
        mut client: AsyncClient,
        req: &DnsRequest,
    ) -> anyhow::Result<()> {
        let rsp = match tokio::time::timeout(
            self.args.timeout,
            client.query(req.name.clone(), req.class, req.rtype),
        )
        .await
        {
            Ok(Ok(rsp)) => rsp,
            Ok(Err(e)) => return Err(anyhow!("failed to query: {e}")),
            Err(_) => return Err(anyhow!("timed out to read query response")),
        };

        if rsp.response_code() != ResponseCode::NoError {
            return Err(anyhow!("Got error response code {}", rsp.response_code()));
        }

        if self.args.dump_result {
            println!("Total {} answers", rsp.answer_count());
            for r in rsp.answers() {
                println!(" {r}");
            }
        }

        Ok(())
    }
}

#[async_trait]
impl BenchTaskContext for DnsTaskContext {
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
        let client = self
            .fetch_client()
            .await
            .context("fetch dns client failed")
            .map_err(BenchError::Fatal)?;
        let req = if self.args.iter_global {
            self.args.fetch_request(&self.args.global_picker)
        } else {
            self.args.fetch_request(&self.local_picker)
        }
        .ok_or_else(|| BenchError::Fatal(anyhow!("no request found")))?;

        match self.run_with_client(client, req).await {
            Ok(_) => {
                let total_time = time_started.elapsed();
                if let Some(r) = &mut self.histogram_recorder {
                    r.record_total_time(total_time);
                }
                Ok(())
            }
            Err(e) => {
                self.drop_client();
                Err(BenchError::Task(e))
            }
        }
    }
}
