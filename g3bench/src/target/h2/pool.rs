/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::anyhow;
use bytes::Bytes;
use h2::client::SendRequest;
use tokio::sync::Mutex;

use super::{BenchH2Args, HttpHistogramRecorder, HttpRuntimeStats, ProcArgs};

struct H2ConnectionUnlocked {
    args: Arc<BenchH2Args>,
    proc_args: Arc<ProcArgs>,
    index: usize,
    h2s: Option<SendRequest<Bytes>>,
    runtime_stats: Arc<HttpRuntimeStats>,
    histogram_recorder: HttpHistogramRecorder,
    reuse_conn_count: u64,
}

impl Drop for H2ConnectionUnlocked {
    fn drop(&mut self) {
        self.histogram_recorder
            .record_conn_reuse_count(self.reuse_conn_count);
        self.reuse_conn_count = 0;
    }
}

impl H2ConnectionUnlocked {
    fn new(
        args: Arc<BenchH2Args>,
        proc_args: Arc<ProcArgs>,
        index: usize,
        runtime_stats: Arc<HttpRuntimeStats>,
        histogram_recorder: HttpHistogramRecorder,
    ) -> Self {
        H2ConnectionUnlocked {
            args,
            proc_args,
            index,
            h2s: None,
            runtime_stats,
            histogram_recorder,
            reuse_conn_count: 0,
        }
    }

    async fn fetch_stream(&mut self) -> anyhow::Result<SendRequest<Bytes>> {
        if let Some(h2s) = self.h2s.clone() {
            if let Ok(send_req) = h2s.ready().await {
                self.reuse_conn_count += 1;
                return Ok(send_req);
            }
        }

        self.histogram_recorder
            .record_conn_reuse_count(self.reuse_conn_count);
        self.reuse_conn_count = 0;

        self.runtime_stats.add_conn_attempt();
        let new_h2s = match tokio::time::timeout(
            self.args.common.connect_timeout,
            self.args
                .new_h2_connection(&self.runtime_stats, &self.proc_args),
        )
        .await
        {
            Ok(Ok(h2s)) => h2s,
            Ok(Err(e)) => return Err(e.context(format!("P#{} new connection failed", self.index))),
            Err(_) => return Err(anyhow!("timeout to get new connection")),
        };
        self.runtime_stats.add_conn_success();
        let s = new_h2s
            .clone()
            .ready()
            .await
            .map_err(|e| anyhow!("P#{} failed to open new stream: {e:?}", self.index))?;
        self.h2s = Some(new_h2s);
        Ok(s)
    }
}

struct H2Connection {
    inner: Mutex<H2ConnectionUnlocked>,
}

impl H2Connection {
    fn new(
        args: Arc<BenchH2Args>,
        proc_args: Arc<ProcArgs>,
        index: usize,
        runtime_stats: Arc<HttpRuntimeStats>,
        histogram_recorder: HttpHistogramRecorder,
    ) -> Self {
        H2Connection {
            inner: Mutex::new(H2ConnectionUnlocked::new(
                args,
                proc_args,
                index,
                runtime_stats,
                histogram_recorder,
            )),
        }
    }

    async fn fetch_stream(&self) -> anyhow::Result<SendRequest<Bytes>> {
        let mut inner = self.inner.lock().await;
        inner.fetch_stream().await
    }
}

pub(super) struct H2ConnectionPool {
    pool: Vec<H2Connection>,
    pool_size: usize,
    cur_index: AtomicUsize,
}

impl H2ConnectionPool {
    pub(super) fn new(
        args: &Arc<BenchH2Args>,
        proc_args: &Arc<ProcArgs>,
        pool_size: usize,
        runtime_stats: &Arc<HttpRuntimeStats>,
        histogram_recorder: &HttpHistogramRecorder,
    ) -> Self {
        let mut pool = Vec::with_capacity(pool_size);
        for i in 0..pool_size {
            pool.push(H2Connection::new(
                args.clone(),
                proc_args.clone(),
                i,
                runtime_stats.clone(),
                histogram_recorder.clone(),
            ));
        }

        H2ConnectionPool {
            pool,
            pool_size,
            cur_index: AtomicUsize::new(0),
        }
    }

    pub(super) async fn fetch_stream(&self) -> anyhow::Result<SendRequest<Bytes>> {
        match self.pool_size {
            0 => Err(anyhow!("no connections configured for this pool")),
            1 => self.pool[0].fetch_stream().await,
            _ => {
                let mut indent = self.cur_index.load(Ordering::Acquire);
                loop {
                    let mut next = indent + 1;
                    if next >= self.pool_size {
                        next = 0;
                    }

                    match self.cur_index.compare_exchange(
                        indent,
                        next,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => return self.pool.get(indent).unwrap().fetch_stream().await,
                        Err(v) => indent = v,
                    }
                }
            }
        }
    }
}
