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

use super::{BenchTarget, BenchTaskContext, ProcArgs};

mod opts;
use opts::BenchSslArgs;

mod stats;
use stats::{SslHistogram, SslHistogramRecorder, SslRuntimeStats};

mod task;
use task::SslTaskContext;

pub const COMMAND: &str = "ssl";

struct SslTarget {
    args: Arc<BenchSslArgs>,
    proc_args: Arc<ProcArgs>,
    stats: Arc<SslRuntimeStats>,
    histogram: Option<SslHistogram>,
}

impl BenchTarget<SslRuntimeStats, SslHistogram, SslTaskContext> for SslTarget {
    fn new_context(&self) -> anyhow::Result<SslTaskContext> {
        let histogram_recorder = self.histogram.as_ref().map(|h| h.recorder());
        SslTaskContext::new(&self.args, &self.proc_args, &self.stats, histogram_recorder)
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

pub async fn run(proc_args: &Arc<ProcArgs>, cmd_args: &ArgMatches) -> anyhow::Result<()> {
    let mut ssl_args = opts::parse_ssl_args(cmd_args)?;
    ssl_args.resolve_target_address(proc_args).await?;

    let target = SslTarget {
        args: Arc::new(ssl_args),
        proc_args: Arc::clone(proc_args),
        stats: Arc::new(SslRuntimeStats::default()),
        histogram: Some(SslHistogram::new()),
    };

    super::run(target, proc_args).await
}
