/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::future::poll_fn;
use std::sync::Arc;

use anyhow::{Context, anyhow};
use bytes::Bytes;
use h2::client::SendRequest;
use http::{Request, Version};
use tokio::time::Instant;

use super::{
    BenchH2Args, BenchTaskContext, H2ConnectionPool, HttpHistogramRecorder, HttpRuntimeStats,
    ProcArgs,
};
use crate::target::BenchError;

pub(super) struct H2TaskContext {
    args: Arc<BenchH2Args>,
    proc_args: Arc<ProcArgs>,

    pool: Option<Arc<H2ConnectionPool>>,
    h2s: Option<SendRequest<Bytes>>,

    reuse_conn_count: u64,
    static_headers: Request<()>,

    runtime_stats: Arc<HttpRuntimeStats>,
    histogram_recorder: HttpHistogramRecorder,
}

impl Drop for H2TaskContext {
    fn drop(&mut self) {
        self.histogram_recorder
            .record_conn_reuse_count(self.reuse_conn_count);
    }
}

impl H2TaskContext {
    pub(super) fn new(
        args: &Arc<BenchH2Args>,
        proc_args: &Arc<ProcArgs>,
        runtime_stats: &Arc<HttpRuntimeStats>,
        histogram_recorder: HttpHistogramRecorder,
        pool: Option<Arc<H2ConnectionPool>>,
    ) -> anyhow::Result<Self> {
        let static_request = args
            .common
            .build_static_request(Version::HTTP_2)
            .context("failed to build static request header")?;
        Ok(H2TaskContext {
            args: Arc::clone(args),
            proc_args: Arc::clone(proc_args),
            pool,
            h2s: None,
            reuse_conn_count: 0,
            static_headers: static_request,
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

        if let Some(h2s) = self.h2s.clone()
            && let Ok(ups_send_req) = h2s.ready().await
        {
            self.reuse_conn_count += 1;
            return Ok(ups_send_req);
        }

        if self.reuse_conn_count > 0 {
            self.histogram_recorder
                .record_conn_reuse_count(self.reuse_conn_count);
            self.reuse_conn_count = 0;
        }

        self.runtime_stats.add_conn_attempt();
        let h2s = match tokio::time::timeout(
            self.args.common.connect_timeout,
            self.args.connect.new_h2_connection(
                &self.args.common.target,
                &self.runtime_stats,
                &self.proc_args,
            ),
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
        let req = self.static_headers.clone();
        let payload = self.args.common.payload();

        // send hdr
        let (rsp_fut, mut send_stream) = send_req
            .send_request(req, payload.is_none())
            .map_err(|e| anyhow!("failed to send request: {e:?}"))?;
        let send_hdr_time = time_started.elapsed();
        self.histogram_recorder.record_send_hdr_time(send_hdr_time);

        if let Some(data) = payload {
            send_stream.reserve_capacity(data.len());
            let mut data = Bytes::from(data.clone());

            loop {
                match poll_fn(|cx| send_stream.poll_capacity(cx)).await {
                    Some(Ok(nw)) => {
                        if nw >= data.len() {
                            send_stream.send_data(data, true)?;
                            self.histogram_recorder
                                .record_send_all_time(time_started.elapsed());
                            break;
                        } else {
                            let to_write = data.split_to(nw);
                            send_stream.send_data(to_write, false)?;
                        }
                    }
                    Some(Err(e)) => return Err(anyhow!("error when poll send capacity: {e}")),
                    None => {
                        return Err(anyhow!("send stream not in send state when poll capacity"));
                    }
                }
            }
        } else {
            self.histogram_recorder.record_send_all_time(send_hdr_time);
        }

        // recv hdr
        let rsp = match tokio::time::timeout(self.args.common.timeout, rsp_fut).await {
            Ok(Ok(rsp)) => rsp,
            Ok(Err(e)) => return Err(anyhow!("failed to read response: {e}")),
            Err(_) => return Err(anyhow!("timeout to read response")),
        };
        let (rsp, mut rsp_recv_body) = rsp.into_parts();
        let recv_hdr_time = time_started.elapsed();
        self.histogram_recorder.record_recv_hdr_time(recv_hdr_time);
        if let Some(ok_status) = self.args.common.ok_status
            && rsp.status != ok_status
        {
            return Err(anyhow!(
                "Got rsp code {} while {} is expected",
                rsp.status.as_u16(),
                ok_status.as_u16()
            ));
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
                self.histogram_recorder.record_total_time(total_time);
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
