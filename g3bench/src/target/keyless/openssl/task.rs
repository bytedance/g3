/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use tokio::time::Instant;

#[cfg(feature = "openssl-async-job")]
use super::KeylessOpensslAsyncJob;
use super::{
    BenchTaskContext, KeylessHistogramRecorder, KeylessOpensslArgs, KeylessRuntimeStats, ProcArgs,
};
use crate::target::BenchError;

pub(super) struct KeylessOpensslTaskContext {
    args: Arc<KeylessOpensslArgs>,
    proc_args: Arc<ProcArgs>,

    runtime_stats: Arc<KeylessRuntimeStats>,
    histogram_recorder: KeylessHistogramRecorder,
}

impl KeylessOpensslTaskContext {
    pub(super) fn new(
        args: &Arc<KeylessOpensslArgs>,
        proc_args: &Arc<ProcArgs>,
        runtime_stats: &Arc<KeylessRuntimeStats>,
        histogram_recorder: KeylessHistogramRecorder,
    ) -> anyhow::Result<Self> {
        Ok(KeylessOpensslTaskContext {
            args: Arc::clone(args),
            proc_args: Arc::clone(proc_args),
            runtime_stats: Arc::clone(runtime_stats),
            histogram_recorder,
        })
    }

    #[cfg(feature = "openssl-async-job")]
    async fn run_action(&self) -> anyhow::Result<Vec<u8>> {
        if self.proc_args.use_unaided_worker {
            KeylessOpensslAsyncJob::new(self.args.clone()).run().await
        } else {
            self.args.handle_action()
        }
    }

    #[cfg(not(feature = "openssl-async-job"))]
    async fn run_action(&self) -> anyhow::Result<Vec<u8>> {
        self.args.handle_action()
    }
}

impl BenchTaskContext for KeylessOpensslTaskContext {
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
        let output = self.run_action().await.map_err(BenchError::Fatal)?;
        let total_time = time_started.elapsed();
        self.histogram_recorder.record_total_time(total_time);
        self.args
            .global
            .check_result(task_id, output, &self.proc_args)
            .map_err(BenchError::Task)?;
        tokio::task::yield_now().await;
        Ok(())
    }
}
