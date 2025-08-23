/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::anyhow;
use tokio::sync::Mutex;

use super::{
    MultiplexTransfer, ProcArgs, ThriftHistogramRecorder, ThriftRuntimeStats, ThriftTcpArgs,
};

struct ThriftConnectionUnlocked {
    args: Arc<ThriftTcpArgs>,
    proc_args: Arc<ProcArgs>,
    index: usize,
    save: Option<Arc<MultiplexTransfer>>,
    runtime_stats: Arc<ThriftRuntimeStats>,
    histogram_recorder: ThriftHistogramRecorder,
    reuse_conn_count: u64,
}

impl Drop for ThriftConnectionUnlocked {
    fn drop(&mut self) {
        self.histogram_recorder
            .record_conn_reuse_count(self.reuse_conn_count);
        self.reuse_conn_count = 0;
    }
}

impl ThriftConnectionUnlocked {
    fn new(
        args: Arc<ThriftTcpArgs>,
        proc_args: Arc<ProcArgs>,
        index: usize,
        runtime_stats: Arc<ThriftRuntimeStats>,
        histogram_recorder: ThriftHistogramRecorder,
    ) -> Self {
        ThriftConnectionUnlocked {
            args,
            proc_args,
            index,
            save: None,
            runtime_stats,
            histogram_recorder,
            reuse_conn_count: 0,
        }
    }

    async fn fetch_handle(&mut self) -> anyhow::Result<Arc<MultiplexTransfer>> {
        if let Some(handle) = &self.save {
            if !handle.is_closed() {
                self.reuse_conn_count += 1;
                return Ok(handle.clone());
            }
            self.save = None;
        }

        self.histogram_recorder
            .record_conn_reuse_count(self.reuse_conn_count);
        self.reuse_conn_count = 0;

        self.runtime_stats.add_conn_attempt();
        let handle = match tokio::time::timeout(
            self.args.connect_timeout,
            self.args.new_multiplex_connection(&self.proc_args),
        )
        .await
        {
            Ok(Ok(h)) => Arc::new(h),
            Ok(Err(e)) => return Err(e.context(format!("P#{} new connection failed", self.index))),
            Err(_) => return Err(anyhow!("timeout to get new connection")),
        };
        self.runtime_stats.add_conn_success();
        self.save = Some(handle.clone());
        Ok(handle)
    }
}

struct ThriftConnection {
    inner: Mutex<ThriftConnectionUnlocked>,
}

impl ThriftConnection {
    fn new(
        args: Arc<ThriftTcpArgs>,
        proc_args: Arc<ProcArgs>,
        index: usize,
        runtime_stats: Arc<ThriftRuntimeStats>,
        histogram_recorder: ThriftHistogramRecorder,
    ) -> Self {
        ThriftConnection {
            inner: Mutex::new(ThriftConnectionUnlocked::new(
                args,
                proc_args,
                index,
                runtime_stats,
                histogram_recorder,
            )),
        }
    }

    async fn fetch_handle(&self) -> anyhow::Result<Arc<MultiplexTransfer>> {
        let mut inner = self.inner.lock().await;
        inner.fetch_handle().await
    }
}

pub(super) struct ThriftConnectionPool {
    pool: Vec<ThriftConnection>,
    pool_size: usize,
    cur_index: AtomicUsize,
}

impl ThriftConnectionPool {
    pub(super) fn new(
        args: &Arc<ThriftTcpArgs>,
        proc_args: &Arc<ProcArgs>,
        pool_size: usize,
        runtime_stats: &Arc<ThriftRuntimeStats>,
        histogram_recorder: &ThriftHistogramRecorder,
    ) -> Self {
        let mut pool = Vec::with_capacity(pool_size);
        for i in 0..pool_size {
            pool.push(ThriftConnection::new(
                args.clone(),
                proc_args.clone(),
                i,
                runtime_stats.clone(),
                histogram_recorder.clone(),
            ));
        }

        ThriftConnectionPool {
            pool,
            pool_size,
            cur_index: AtomicUsize::new(0),
        }
    }

    pub(super) async fn fetch_handle(&self) -> anyhow::Result<Arc<MultiplexTransfer>> {
        match self.pool_size {
            0 => Err(anyhow!("no connections configured for this pool")),
            1 => self.pool[0].fetch_handle().await,
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
                        Ok(_) => return self.pool.get(indent).unwrap().fetch_handle().await,
                        Err(v) => indent = v,
                    }
                }
            }
        }
    }
}
