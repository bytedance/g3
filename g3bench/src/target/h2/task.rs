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
use h2::client::SendRequest;
use tokio::time::Instant;

use super::{
    BenchH2Args, BenchTaskContext, H2ConnectionPool, H2PreRequest, HttpHistogramRecorder,
    HttpRuntimeStats, ProcArgs,
};
use crate::target::BenchError;

pub(super) struct H2TaskContext {
    args: Arc<BenchH2Args>,
    proc_args: Arc<ProcArgs>,

    pool: Option<Arc<H2ConnectionPool>>,
    h2s: Option<SendRequest<Bytes>>,

    reuse_conn_count: u64,
    pre_request: H2PreRequest,

    runtime_stats: Arc<HttpRuntimeStats>,
    histogram_recorder: Option<HttpHistogramRecorder>,
}

impl Drop for H2TaskContext {
    fn drop(&mut self) {
        if let Some(r) = &mut self.histogram_recorder {
            r.record_conn_reuse_count(self.reuse_conn_count);
        }
    }
}

impl H2TaskContext {
    pub(super) fn new(
        args: &Arc<BenchH2Args>,
        proc_args: &Arc<ProcArgs>,
        runtime_stats: &Arc<HttpRuntimeStats>,
        histogram_recorder: Option<HttpHistogramRecorder>,
        pool: Option<Arc<H2ConnectionPool>>,
    ) -> anyhow::Result<Self> {
        let pre_request = args
            .build_pre_request_header()
            .context("failed to build request header")?;
        Ok(H2TaskContext {
            args: Arc::clone(args),
            proc_args: Arc::clone(proc_args),
            pool,
            h2s: None,
            reuse_conn_count: 0,
            pre_request,
            runtime_stats: Arc::clone(runtime_stats),
            histogram_recorder,
        })
    }

    fn drop_connection(&mut self) {
        self.h2s = None;
    }

    async fn fetch_stream(&mut self) -> anyhow::Result<SendRequest<Bytes>> {
        if let Some(pool) = &self.pool {
            return pool.fetch_stream().await;
        }

        if let Some(h2s) = self.h2s.clone() {
            if let Ok(ups_send_req) = h2s.ready().await {
                self.reuse_conn_count += 1;
                return Ok(ups_send_req);
            }
        }

        if self.reuse_conn_count > 0 {
            if let Some(r) = &mut self.histogram_recorder {
                r.record_conn_reuse_count(self.reuse_conn_count);
            }
            self.reuse_conn_count = 0;
        }

        self.runtime_stats.add_conn_attempt();
        let h2s = match tokio::time::timeout(
            self.args.connect_timeout,
            self.args
                .new_h2_connection(&self.runtime_stats, &self.proc_args),
        )
        .await
        {
            Ok(Ok(h2s)) => h2s,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(anyhow!("timeout to get new connection")),
        };
        self.runtime_stats.add_conn_success();

        let s = h2s
            .clone()
            .ready()
            .await
            .map_err(|e| anyhow!("failed to open new stream on new connection: {e:?}"))?;
        self.h2s = Some(h2s);
        Ok(s)
    }

    async fn run_with_stream(
        &mut self,
        time_started: Instant,
        mut send_req: SendRequest<Bytes>,
    ) -> anyhow::Result<()> {
        let req = self
            .pre_request
            .build_request()
            .context("failed to build request header")?;

        // send hdr
        let (rsp_fut, _) = send_req
            .send_request(req, true)
            .map_err(|e| anyhow!("failed to send request: {e:?}"))?;
        let send_hdr_time = time_started.elapsed();
        if let Some(r) = &mut self.histogram_recorder {
            r.record_send_hdr_time(send_hdr_time);
        }

        // recv hdr
        let rsp = match tokio::time::timeout(self.args.timeout, rsp_fut).await {
            Ok(Ok(rsp)) => rsp,
            Ok(Err(e)) => return Err(anyhow!("failed to read response: {e}")),
            Err(_) => return Err(anyhow!("timeout to read response")),
        };
        let (rsp, mut rsp_recv_body) = rsp.into_parts();
        let recv_hdr_time = time_started.elapsed();
        if let Some(r) = &mut self.histogram_recorder {
            r.record_recv_hdr_time(recv_hdr_time);
        }
        if let Some(ok_status) = self.args.ok_status {
            if rsp.status != ok_status {
                return Err(anyhow!(
                    "Got rsp code {} while {} is expected",
                    rsp.status.as_u16(),
                    ok_status.as_u16()
                ));
            }
        }

        // recv body
        if !rsp_recv_body.is_end_stream() {
            while let Some(r) = rsp_recv_body.data().await {
                match r {
                    Ok(bytes) => {
                        rsp_recv_body
                            .flow_control()
                            .release_capacity(bytes.len())
                            .map_err(|e| {
                                anyhow!("failed to release capacity while reading body: {e:?}")
                            })?;
                    }
                    Err(e) => {
                        return Err(anyhow!("failed to recv rsp body: {e:?}"));
                    }
                }
            }
            let _ = rsp_recv_body
                .trailers()
                .await
                .map_err(|e| anyhow!("failed to recv rsp trailers: {e:?}"))?;
        }

        Ok(())
    }
}

#[async_trait]
impl BenchTaskContext for H2TaskContext {
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
