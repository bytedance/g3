/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::process::ExitCode;
use std::sync::Arc;

use clap::{ArgMatches, Command};

use super::{WebsocketHistogram, WebsocketHistogramRecorder};
use crate::ProcArgs;
use crate::module::http::HttpRuntimeStats;
use crate::target::BenchTarget;

mod opts;
use opts::H1WebsocketArgs;

mod task;
use task::H1WebsocketTaskContext;

pub(super) const COMMAND: &str = "h1";

struct H1WebsocketTarget {
    args: Arc<H1WebsocketArgs>,
    proc_args: Arc<ProcArgs>,
    stats: Arc<HttpRuntimeStats>,
    histogram: Option<WebsocketHistogram>,
    histogram_recorder: WebsocketHistogramRecorder,
}

impl BenchTarget<HttpRuntimeStats, WebsocketHistogram, H1WebsocketTaskContext>
    for H1WebsocketTarget
{
    fn new_context(&self) -> anyhow::Result<H1WebsocketTaskContext> {
        Ok(H1WebsocketTaskContext::new(
            self.args.clone(),
            self.proc_args.clone(),
            self.stats.clone(),
            self.histogram_recorder.clone(),
        ))
    }

    fn fetch_runtime_stats(&self) -> Arc<HttpRuntimeStats> {
        self.stats.clone()
    }

    fn take_histogram(&mut self) -> Option<WebsocketHistogram> {
        self.histogram.take()
    }
}

pub(super) fn command() -> Command {
    opts::add_h1_websocket_args(Command::new(COMMAND).about("Test websocket over http1.1"))
}

pub async fn run(proc_args: &Arc<ProcArgs>, cmd_args: &ArgMatches) -> anyhow::Result<ExitCode> {
    let mut websocket_args = opts::parse_h1_websocket_args(cmd_args)?;
    websocket_args
        .connect
        .resolve_target_address(proc_args, &websocket_args.common.target)
        .await?;

    let (histogram, histogram_recorder) = WebsocketHistogram::new();
    let target = H1WebsocketTarget {
        args: Arc::new(websocket_args),
        proc_args: Arc::clone(proc_args),
        stats: Arc::new(HttpRuntimeStats::new_tcp(COMMAND)),
        histogram: Some(histogram),
        histogram_recorder,
    };

    crate::target::run(target, proc_args).await
}
