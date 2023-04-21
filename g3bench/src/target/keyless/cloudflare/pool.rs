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
use tokio::sync::Mutex;

use super::{
    KeylessCloudflareArgs, KeylessHistogram, KeylessHistogramRecorder, KeylessRuntimeStats,
    ProcArgs, SendHandle,
};

struct KeylessConnectionUnlocked {
    args: Arc<KeylessCloudflareArgs>,
    proc_args: Arc<ProcArgs>,
    index: usize,
    save: Option<SendHandle>,
    runtime_stats: Arc<KeylessRuntimeStats>,
    histogram_recorder: Option<KeylessHistogramRecorder>,
    reuse_conn_count: u64,
}

impl Drop for KeylessConnectionUnlocked {
    fn drop(&mut self) {
        if let Some(r) = &mut self.histogram_recorder {
            r.record_conn_reuse_count(self.reuse_conn_count);
        }
        self.reuse_conn_count = 0;
    }
}

impl KeylessConnectionUnlocked {
    fn new(
        args: Arc<KeylessCloudflareArgs>,
        proc_args: Arc<ProcArgs>,
        index: usize,
        runtime_stats: Arc<KeylessRuntimeStats>,
        histogram_recorder: Option<KeylessHistogramRecorder>,
    ) -> Self {
        KeylessConnectionUnlocked {
            args,
            proc_args,
            index,
            save: None,
            runtime_stats,
            histogram_recorder,
            reuse_conn_count: 0,
        }
    }

    async fn fetch_handle(&mut self) -> anyhow::Result<SendHandle> {
        if let Some(handle) = &self.save {
            if !handle.is_closed() {
                self.reuse_conn_count += 1;
                return Ok(handle.clone());
            }
            self.save = None;
        }

        if let Some(r) = &mut self.histogram_recorder {
            r.record_conn_reuse_count(self.reuse_conn_count);
        }
        self.reuse_conn_count = 0;

        self.runtime_stats.add_conn_attempt();
        let handle = match tokio::time::timeout(
            self.args.connect_timeout,
            self.args.new_keyless_connection(&self.proc_args),
        )
        .await
        {
            Ok(Ok(h)) => h,
            Ok(Err(e)) => return Err(e.context(format!("P#{} new connection failed", self.index))),
            Err(_) => return Err(anyhow!("timeout to get new connection")),
        };
        self.runtime_stats.add_conn_success();
        self.save = Some(handle.clone());
        Ok(handle)
    }
}

struct KeylessConnection {
    inner: Mutex<KeylessConnectionUnlocked>,
}

impl KeylessConnection {
    fn new(
        args: Arc<KeylessCloudflareArgs>,
        proc_args: Arc<ProcArgs>,
        index: usize,
        runtime_stats: Arc<KeylessRuntimeStats>,
        histogram_recorder: Option<KeylessHistogramRecorder>,
    ) -> Self {
        KeylessConnection {
            inner: Mutex::new(KeylessConnectionUnlocked::new(
                args,
                proc_args,
                index,
                runtime_stats,
                histogram_recorder,
            )),
        }
    }

    async fn fetch_handle(&self) -> anyhow::Result<SendHandle> {
        let mut inner = self.inner.lock().await;
        inner.fetch_handle().await
    }
}

pub(super) struct KeylessConnectionPool {
    pool: Vec<KeylessConnection>,
    pool_size: usize,
    cur_index: AtomicUsize,
}

impl KeylessConnectionPool {
    pub(super) fn new(
        args: &Arc<KeylessCloudflareArgs>,
        proc_args: &Arc<ProcArgs>,
        pool_size: usize,
        runtime_stats: &Arc<KeylessRuntimeStats>,
        histogram_stats: Option<&KeylessHistogram>,
    ) -> Self {
        let mut pool = Vec::with_capacity(pool_size);
        for i in 0..pool_size {
            pool.push(KeylessConnection::new(
                args.clone(),
                proc_args.clone(),
                i,
                runtime_stats.clone(),
                histogram_stats.map(|s| s.recorder()),
            ));
        }

        KeylessConnectionPool {
            pool,
            pool_size,
            cur_index: AtomicUsize::new(0),
        }
    }

    pub(super) async fn fetch_handle(&self) -> anyhow::Result<SendHandle> {
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
