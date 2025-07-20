/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::process::ExitCode;
use std::sync::Arc;

use clap::{ArgMatches, Command};

use crate::ProcArgs;
use crate::target::BenchTarget;

mod header;

mod opts;
use opts::ThriftTcpArgs;

mod stats;
use stats::{ThriftHistogram, ThriftHistogramRecorder, ThriftRuntimeStats};

mod pool;
use pool::ThriftConnectionPool;

mod connection;
use connection::{
    MultiplexTransfer, SimplexTransfer, ThriftTcpRequest, ThriftTcpResponse,
    ThriftTcpResponseError, ThriftTcpResponseLocalError,
};

mod task;
use task::ThriftTcpTaskContext;

pub(super) const COMMAND: &str = "tcp";

struct ThriftTcpTarget {
    args: Arc<ThriftTcpArgs>,
    proc_args: Arc<ProcArgs>,
    stats: Arc<ThriftRuntimeStats>,
    histogram: Option<ThriftHistogram>,
    histogram_recorder: ThriftHistogramRecorder,
    pool: Option<Arc<ThriftConnectionPool>>,
}

impl BenchTarget<ThriftRuntimeStats, ThriftHistogram, ThriftTcpTaskContext> for ThriftTcpTarget {
    fn new_context(&self) -> anyhow::Result<ThriftTcpTaskContext> {
        ThriftTcpTaskContext::new(
            &self.args,
            &self.proc_args,
            &self.stats,
            self.histogram_recorder.clone(),
            self.pool.clone(),
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

    let args = Arc::new(args);

    let stats = Arc::new(ThriftRuntimeStats::default());
    let (histogram, histogram_recorder) = ThriftHistogram::new();

    let pool = args.pool_size.map(|s| {
        Arc::new(ThriftConnectionPool::new(
            &args,
            proc_args,
            s,
            &stats,
            &histogram_recorder,
        ))
    });

    let target = ThriftTcpTarget {
        args,
        proc_args: proc_args.clone(),
        stats,
        histogram: Some(histogram),
        histogram_recorder,
        pool,
    };

    crate::target::run(target, proc_args).await
}
