/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::process::ExitCode;
use std::sync::Arc;

use clap::{ArgMatches, Command};

use crate::ProcArgs;
use crate::target::BenchTarget;

mod opts;
use opts::ThriftTcpArgs;

mod stats;
use stats::{ThriftHistogram, ThriftHistogramRecorder, ThriftRuntimeStats};

mod connection;
use connection::ThriftConnection;

mod task;
use task::ThriftTcpTaskContext;

pub(super) const COMMAND: &str = "tcp";

struct ThriftTcpTarget {
    args: Arc<ThriftTcpArgs>,
    proc_args: Arc<ProcArgs>,
    stats: Arc<ThriftRuntimeStats>,
    histogram: Option<ThriftHistogram>,
    histogram_recorder: ThriftHistogramRecorder,
}

impl BenchTarget<ThriftRuntimeStats, ThriftHistogram, ThriftTcpTaskContext> for ThriftTcpTarget {
    fn new_context(&self) -> anyhow::Result<ThriftTcpTaskContext> {
        ThriftTcpTaskContext::new(
            &self.args,
            &self.proc_args,
            &self.stats,
            self.histogram_recorder.clone(),
        )
    }

    fn fetch_runtime_stats(&self) -> Arc<ThriftRuntimeStats> {
        self.stats.clone()
    }

    fn take_histogram(&mut self) -> Option<ThriftHistogram> {
        self.histogram.take()
    }
}

pub(super) fn command() -> Command {
    opts::add_tcp_args(Command::new(COMMAND).about("Test thrift over tcp transport"))
}

pub(super) async fn run(
    proc_args: &Arc<ProcArgs>,
    cmd_args: &ArgMatches,
) -> anyhow::Result<ExitCode> {
    let mut args = opts::parse_tcp_args(cmd_args)?;
    args.resolve_target_address(proc_args).await?;

    let stats = Arc::new(ThriftRuntimeStats::default());
    let (histogram, histogram_recorder) = ThriftHistogram::new();

    let target = ThriftTcpTarget {
        args: Arc::new(args),
        proc_args: proc_args.clone(),
        stats,
        histogram: Some(histogram),
        histogram_recorder,
    };

    crate::target::run(target, proc_args).await
}
