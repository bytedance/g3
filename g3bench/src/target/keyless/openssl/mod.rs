/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::process::ExitCode;
use std::sync::Arc;

use clap::{ArgMatches, Command};

use super::{BenchTarget, BenchTaskContext};
use crate::opts::ProcArgs;

mod stats;
use stats::{KeylessHistogram, KeylessHistogramRecorder, KeylessRuntimeStats};

mod task;
use task::KeylessOpensslTaskContext;

mod opts;
use opts::KeylessOpensslArgs;

#[cfg(feature = "openssl-async-job")]
mod async_job;
#[cfg(feature = "openssl-async-job")]
use async_job::KeylessOpensslAsyncJob;

pub(super) const COMMAND: &str = "openssl";

struct KeylessOpensslTarget {
    args: Arc<KeylessOpensslArgs>,
    proc_args: Arc<ProcArgs>,
    stats: Arc<KeylessRuntimeStats>,
    histogram: Option<KeylessHistogram>,
    histogram_recorder: KeylessHistogramRecorder,
}

impl BenchTarget<KeylessRuntimeStats, KeylessHistogram, KeylessOpensslTaskContext>
    for KeylessOpensslTarget
{
    fn new_context(&self) -> anyhow::Result<KeylessOpensslTaskContext> {
        KeylessOpensslTaskContext::new(
            &self.args,
            &self.proc_args,
            &self.stats,
            self.histogram_recorder.clone(),
        )
    }

    fn fetch_runtime_stats(&self) -> Arc<KeylessRuntimeStats> {
        self.stats.clone()
    }

    fn take_histogram(&mut self) -> Option<KeylessHistogram> {
        self.histogram.take()
    }
}

pub(super) fn command() -> Command {
    opts::add_openssl_args(
        Command::new(COMMAND).about("Use local openssl instead of keyless servers"),
    )
}

pub(super) async fn run(
    proc_args: &Arc<ProcArgs>,
    cmd_args: &ArgMatches,
) -> anyhow::Result<ExitCode> {
    let global_args = opts::parse_openssl_args(cmd_args)?;

    let runtime_stats = Arc::new(KeylessRuntimeStats::default());
    let (histogram, histogram_recorder) = KeylessHistogram::new();

    let target = KeylessOpensslTarget {
        args: Arc::new(global_args),
        proc_args: Arc::clone(proc_args),
        stats: runtime_stats,
        histogram: Some(histogram),
        histogram_recorder,
    };

    crate::target::run(target, proc_args).await
}
