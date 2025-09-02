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
use opts::BenchHttpArgs;

mod task;
use task::HttpTaskContext;

pub const COMMAND: &str = "h1";

struct HttpTarget {
    args: Arc<BenchHttpArgs>,
    proc_args: Arc<ProcArgs>,
    stats: Arc<HttpRuntimeStats>,
    histogram: Option<HttpHistogram>,
    histogram_recorder: HttpHistogramRecorder,
}

impl BenchTarget<HttpRuntimeStats, HttpHistogram, HttpTaskContext> for HttpTarget {
    fn new_context(&self) -> anyhow::Result<HttpTaskContext> {
        HttpTaskContext::new(
            self.args.clone(),
            self.proc_args.clone(),
            self.stats.clone(),
            self.histogram_recorder.clone(),
        )
    }

    fn fetch_runtime_stats(&self) -> Arc<HttpRuntimeStats> {
        self.stats.clone()
    }

    fn take_histogram(&mut self) -> Option<HttpHistogram> {
        self.histogram.take()
    }
}

pub fn command() -> Command {
    opts::add_http_args(Command::new(COMMAND))
}

pub async fn run(proc_args: &Arc<ProcArgs>, cmd_args: &ArgMatches) -> anyhow::Result<ExitCode> {
    let mut http_args = opts::parse_http_args(cmd_args)?;
    http_args
        .connect
        .resolve_target_address(proc_args, &http_args.common.target)
        .await?;

    let (histogram, histogram_recorder) = HttpHistogram::new();
    let target = HttpTarget {
        args: Arc::new(http_args),
        proc_args: Arc::clone(proc_args),
        stats: Arc::new(HttpRuntimeStats::new_tcp(COMMAND)),
        histogram: Some(histogram),
        histogram_recorder,
    };

    super::run(target, proc_args).await
}
