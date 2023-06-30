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

mod async_job;
use async_job::KeylessOpensslAsyncJob;

pub(super) const COMMAND: &str = "openssl";

struct KeylessOpensslTarget {
    args: Arc<KeylessOpensslArgs>,
    proc_args: Arc<ProcArgs>,
    stats: Arc<KeylessRuntimeStats>,
    histogram: Option<KeylessHistogram>,
}

impl BenchTarget<KeylessRuntimeStats, KeylessHistogram, KeylessOpensslTaskContext>
    for KeylessOpensslTarget
{
    fn new_context(&self) -> anyhow::Result<KeylessOpensslTaskContext> {
        let histogram_recorder = self.histogram.as_ref().map(|h| h.recorder());
        KeylessOpensslTaskContext::new(&self.args, &self.proc_args, &self.stats, histogram_recorder)
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

pub(super) async fn run(proc_args: &Arc<ProcArgs>, cmd_args: &ArgMatches) -> anyhow::Result<()> {
    let global_args = opts::parse_openssl_args(cmd_args)?;

    let runtime_stats = Arc::new(KeylessRuntimeStats::default());
    let histogram = Some(KeylessHistogram::new());

    let target = KeylessOpensslTarget {
        args: Arc::new(global_args),
        proc_args: Arc::clone(proc_args),
        stats: runtime_stats,
        histogram,
    };

    crate::target::run(target, proc_args).await
}
