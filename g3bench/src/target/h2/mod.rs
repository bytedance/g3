/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::process::ExitCode;
use std::sync::Arc;

use clap::{ArgMatches, Command};

use super::{BenchTarget, BenchTaskContext, ProcArgs};
use crate::module::http::{HttpHistogram, HttpHistogramRecorder, HttpRuntimeStats};

mod opts;
use opts::BenchH2Args;

mod pool;
use pool::H2ConnectionPool;

mod task;
use task::H2TaskContext;

pub const COMMAND: &str = "h2";

struct H2Target {
    args: Arc<BenchH2Args>,
    proc_args: Arc<ProcArgs>,
    stats: Arc<HttpRuntimeStats>,
    histogram: Option<HttpHistogram>,
    histogram_recorder: HttpHistogramRecorder,
    pool: Option<Arc<H2ConnectionPool>>,
}

impl BenchTarget<HttpRuntimeStats, HttpHistogram, H2TaskContext> for H2Target {
    fn new_context(&self) -> anyhow::Result<H2TaskContext> {
        H2TaskContext::new(
            &self.args,
            &self.proc_args,
            &self.stats,
            self.histogram_recorder.clone(),
            self.pool.clone(),
        )
    }

    fn fetch_runtime_stats(&self) -> Arc<HttpRuntimeStats> {
        self.stats.clone()
    }

    fn take_histogram(&mut self) -> Option<HttpHistogram> {
        self.histogram.take()
    }

    fn notify_finish(&mut self) {
        self.pool = None;
    }
}

pub fn command() -> Command {
    opts::add_h2_args(Command::new(COMMAND))
}

pub async fn run(proc_args: &Arc<ProcArgs>, cmd_args: &ArgMatches) -> anyhow::Result<ExitCode> {
    let mut h2_args = opts::parse_h2_args(cmd_args)?;
    h2_args.resolve_target_address(proc_args).await?;
    let h2_args = Arc::new(h2_args);

    let runtime_stats = Arc::new(HttpRuntimeStats::new_tcp(COMMAND));
    let (histogram, histogram_recorder) = HttpHistogram::new();

    let pool = h2_args.pool_size.map(|s| {
        Arc::new(H2ConnectionPool::new(
            &h2_args,
            proc_args,
            s,
            &runtime_stats,
            &histogram_recorder,
        ))
    });

    let target = H2Target {
        args: h2_args,
        proc_args: Arc::clone(proc_args),
        stats: runtime_stats,
        histogram: Some(histogram),
        histogram_recorder,
        pool,
    };

    super::run(target, proc_args).await
}
