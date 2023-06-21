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

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::anyhow;
use bytes::Bytes;
use h3::client::SendRequest;
use h3_quinn::OpenStreams;
use tokio::sync::Mutex;

use super::{BenchH3Args, HttpHistogram, HttpHistogramRecorder, HttpRuntimeStats, ProcArgs};

struct H3ConnectionUnlocked {
    args: Arc<BenchH3Args>,
    proc_args: Arc<ProcArgs>,
    index: usize,
    h3s: Option<SendRequest<OpenStreams, Bytes>>,
    runtime_stats: Arc<HttpRuntimeStats>,
    histogram_recorder: Option<HttpHistogramRecorder>,
    reuse_conn_count: u64,
}

impl Drop for H3ConnectionUnlocked {
    fn drop(&mut self) {
        if let Some(r) = &mut self.histogram_recorder {
            r.record_conn_reuse_count(self.reuse_conn_count);
        }
        self.reuse_conn_count = 0;
    }
}

impl H3ConnectionUnlocked {
    fn new(
        args: Arc<BenchH3Args>,
        proc_args: Arc<ProcArgs>,
        index: usize,
        runtime_stats: Arc<HttpRuntimeStats>,
        histogram_recorder: Option<HttpHistogramRecorder>,
    ) -> Self {
        H3ConnectionUnlocked {
            args,
            proc_args,
            index,
            h3s: None,
            runtime_stats,
            histogram_recorder,
            reuse_conn_count: 0,
        }
    }

    async fn fetch_stream(&mut self) -> anyhow::Result<SendRequest<OpenStreams, Bytes>> {
        if let Some(h3s) = self.h3s.clone() {
            // TODO check close
            self.reuse_conn_count += 1;
            return Ok(h3s);
        }

        if let Some(r) = &mut self.histogram_recorder {
            r.record_conn_reuse_count(self.reuse_conn_count);
        }
        self.reuse_conn_count = 0;

        self.runtime_stats.add_conn_attempt();
        let new_h3s = match tokio::time::timeout(
            self.args.connect_timeout,
            self.args
                .new_h3_connection(&self.runtime_stats, &self.proc_args),
        )
        .await
        {
            Ok(Ok(h3s)) => h3s,
            Ok(Err(e)) => return Err(e.context(format!("P#{} new connection failed", self.index))),
            Err(_) => return Err(anyhow!("timeout to get new connection")),
        };
        self.runtime_stats.add_conn_success();
        let s = new_h3s.clone();
        self.h3s = Some(new_h3s);
        Ok(s)
    }
}

struct H3Connection {
    inner: Mutex<H3ConnectionUnlocked>,
}

impl H3Connection {
    fn new(
        args: Arc<BenchH3Args>,
        proc_args: Arc<ProcArgs>,
        index: usize,
        runtime_stats: Arc<HttpRuntimeStats>,
        histogram_recorder: Option<HttpHistogramRecorder>,
    ) -> Self {
        H3Connection {
            inner: Mutex::new(H3ConnectionUnlocked::new(
                args,
                proc_args,
                index,
                runtime_stats,
                histogram_recorder,
            )),
        }
    }

    async fn fetch_stream(&self) -> anyhow::Result<SendRequest<OpenStreams, Bytes>> {
        let mut inner = self.inner.lock().await;
        inner.fetch_stream().await
    }
}

pub(super) struct H3ConnectionPool {
    pool: Vec<H3Connection>,
    pool_size: usize,
    cur_index: AtomicUsize,
}

impl H3ConnectionPool {
    pub(super) fn new(
        args: &Arc<BenchH3Args>,
        proc_args: &Arc<ProcArgs>,
        pool_size: usize,
        runtime_stats: &Arc<HttpRuntimeStats>,
        histogram_stats: Option<&HttpHistogram>,
    ) -> Self {
        let mut pool = Vec::with_capacity(pool_size);
        for i in 0..pool_size {
            pool.push(H3Connection::new(
                args.clone(),
                proc_args.clone(),
                i,
                runtime_stats.clone(),
                histogram_stats.map(|s| s.recorder()),
            ));
        }

        H3ConnectionPool {
            pool,
            pool_size,
            cur_index: AtomicUsize::new(0),
        }
    }

    pub(super) async fn fetch_stream(&self) -> anyhow::Result<SendRequest<OpenStreams, Bytes>> {
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
