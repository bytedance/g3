/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::process::ExitCode;
use std::sync::Arc;

use clap::{ArgMatches, Command};

use super::{BenchTarget, BenchTaskContext, ProcArgs};
use crate::module::ssl::{SslHistogram, SslHistogramRecorder, SslRuntimeStats};

mod opts;
use opts::BenchRustlsArgs;

mod task;
use task::RustlsTaskContext;

pub const COMMAND: &str = "rustls";

struct RustlsTarget {
    args: Arc<BenchRustlsArgs>,
    proc_args: Arc<ProcArgs>,
    stats: Arc<SslRuntimeStats>,
    histogram: Option<SslHistogram>,
    histogram_recorder: SslHistogramRecorder,
}

impl BenchTarget<SslRuntimeStats, SslHistogram, RustlsTaskContext> for RustlsTarget {
    fn new_context(&self) -> anyhow::Result<RustlsTaskContext> {
        RustlsTaskContext::new(
            &self.args,
            &self.proc_args,
            &self.stats,
            self.histogram_recorder.clone(),
        )
    }

    fn fetch_runtime_stats(&self) -> Arc<SslRuntimeStats> {
        self.stats.clone()
    }

    fn take_histogram(&mut self) -> Option<SslHistogram> {
        self.histogram.take()
    }
}

pub fn command() -> Command {
    opts::add_ssl_args(Command::new(COMMAND))
}

pub async fn run(proc_args: &Arc<ProcArgs>, cmd_args: &ArgMatches) -> anyhow::Result<ExitCode> {
    let mut ssl_args = opts::parse_ssl_args(cmd_args)?;
    ssl_args.resolve_target_address(proc_args).await?;

    let (histogram, histogram_recorder) = SslHistogram::new();
    let target = RustlsTarget {
        args: Arc::new(ssl_args),
        proc_args: Arc::clone(proc_args),
        stats: Arc::new(SslRuntimeStats::default()),
        histogram: Some(histogram),
        histogram_recorder,
    };

    super::run(target, proc_args).await
}
