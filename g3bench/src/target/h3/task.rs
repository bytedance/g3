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
use bytes::Bytes;
use h3::client::SendRequest;
use h3_quinn::OpenStreams;
use tokio::time::Instant;

use super::{
    BenchH3Args, BenchTaskContext, H3ConnectionPool, H3PreRequest, HttpHistogramRecorder,
    HttpRuntimeStats, ProcArgs,
};
use crate::target::BenchError;

pub(super) struct H3TaskContext {
    args: Arc<BenchH3Args>,
    proc_args: Arc<ProcArgs>,

    pool: Option<Arc<H3ConnectionPool>>,
    h3s: Option<SendRequest<OpenStreams, Bytes>>,

    reuse_conn_count: u64,
    pre_request: H3PreRequest,

    runtime_stats: Arc<HttpRuntimeStats>,
    histogram_recorder: Option<HttpHistogramRecorder>,
}

impl Drop for H3TaskContext {
    fn drop(&mut self) {
        if let Some(r) = &mut self.histogram_recorder {
            r.record_conn_reuse_count(self.reuse_conn_count);
        }
    }
}

impl H3TaskContext {
    pub(super) fn new(
        args: &Arc<BenchH3Args>,
        proc_args: &Arc<ProcArgs>,
        runtime_stats: &Arc<HttpRuntimeStats>,
        histogram_recorder: Option<HttpHistogramRecorder>,
        pool: Option<Arc<H3ConnectionPool>>,
    ) -> anyhow::Result<Self> {
        let pre_request = args
            .build_pre_request_header()
            .context("failed to build request header")?;
        Ok(H3TaskContext {
            args: Arc::clone(args),
            proc_args: Arc::clone(proc_args),
            pool,
            h3s: None,
            reuse_conn_count: 0,
            pre_request,
            runtime_stats: Arc::clone(runtime_stats),
            histogram_recorder,
        })
    }

    fn drop_connection(&mut self) {
        self.h3s = None;
    }

    async fn fetch_stream(&mut self) -> anyhow::Result<SendRequest<OpenStreams, Bytes>> {
        if let Some(pool) = &self.pool {
            return pool.fetch_stream().await;
        }

        if let Some(h3s) = self.h3s.clone() {
            // TODO check close
            self.reuse_conn_count += 1;
            return Ok(h3s);
        }

        if self.reuse_conn_count > 0 {
            if let Some(r) = &mut self.histogram_recorder {
                r.record_conn_reuse_count(self.reuse_conn_count);
            }
            self.reuse_conn_count = 0;
        }

        self.runtime_stats.add_conn_attempt();
        let h3s = match tokio::time::timeout(
            self.args.connect_timeout,
            self.args
                .new_h3_connection(&self.runtime_stats, &self.proc_args),
        )
        .await
        {
            Ok(Ok(h3s)) => h3s,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(anyhow!("timeout to get new connection")),
        };
        self.runtime_stats.add_conn_success();

        let s = h3s.clone();
        self.h3s = Some(h3s);
        Ok(s)
    }

    async fn run_with_stream(
        &mut self,
        time_started: Instant,
        mut send_req: SendRequest<OpenStreams, Bytes>,
    ) -> anyhow::Result<()> {
        let req = self
            .pre_request
            .build_request()
            .context("failed to build request header")?;

        // send hdr
        let mut send_stream = send_req
            .send_request(req)
            .await
            .map_err(|e| anyhow!("failed to send request header: {e}"))?;
        send_stream.finish().await?;
        let send_hdr_time = time_started.elapsed();
        if let Some(r) = &mut self.histogram_recorder {
            r.record_send_hdr_time(send_hdr_time);
        }

        // recv hdr
        let rsp = match tokio::time::timeout(self.args.timeout, send_stream.recv_response()).await {
            Ok(Ok(rsp)) => rsp,
            Ok(Err(e)) => return Err(anyhow!("failed to read response: {e}")),
            Err(_) => return Err(anyhow!("timeout to read response")),
        };
        let recv_hdr_time = time_started.elapsed();
        if let Some(r) = &mut self.histogram_recorder {
            r.record_recv_hdr_time(recv_hdr_time);
        }
        if let Some(ok_status) = self.args.ok_status {
            let status = rsp.status();
            if status != ok_status {
                return Err(anyhow!(
                    "Got rsp code {} while {} is expected",
                    status.as_u16(),
                    ok_status.as_u16()
                ));
            }
        }

        // recv body
        while send_stream
            .recv_data()
            .await
            .map_err(|e| anyhow!("failed to recv data: {e}"))?
            .is_some()
        {}
        let _ = send_stream
            .recv_trailers()
            .await
            .map_err(|e| anyhow!("failed to recv trailer: {e}"))?;

        Ok(())
    }
}

#[async_trait]
impl BenchTaskContext for H3TaskContext {
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
        let send_req = self
            .fetch_stream()
            .await
            .context("fetch new stream failed")
            .map_err(BenchError::Fatal)?;

        match self.run_with_stream(time_started, send_req).await {
            Ok(_) => {
                let total_time = time_started.elapsed();
                if let Some(r) = &mut self.histogram_recorder {
                    r.record_total_time(total_time);
                }
                if self.args.no_multiplex {
                    self.drop_connection();
                }
                Ok(())
            }
            Err(e) => {
                self.drop_connection();
                Err(BenchError::Task(e))
            }
        }
    }
}
