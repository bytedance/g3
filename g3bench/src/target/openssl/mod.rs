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
use crate::module::ssl::{SslHistogram, SslHistogramRecorder, SslRuntimeStats};

mod opts;
use opts::BenchOpensslArgs;

mod task;
use task::OpensslTaskContext;

pub const COMMAND: &str = "openssl";

struct OpensslTarget {
    args: Arc<BenchOpensslArgs>,
    proc_args: Arc<ProcArgs>,
    stats: Arc<SslRuntimeStats>,
    histogram: Option<SslHistogram>,
    histogram_recorder: SslHistogramRecorder,
}

impl BenchTarget<SslRuntimeStats, SslHistogram, OpensslTaskContext> for OpensslTarget {
    fn new_context(&self) -> anyhow::Result<OpensslTaskContext> {
        OpensslTaskContext::new(
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

pub async fn run(proc_args: &Arc<ProcArgs>, cmd_args: &ArgMatches) -> anyhow::Result<()> {
    let mut ssl_args = opts::parse_ssl_args(cmd_args)?;
    ssl_args.resolve_target_address(proc_args).await?;

    let (histogram, histogram_recorder) = SslHistogram::new();
    let target = OpensslTarget {
        args: Arc::new(ssl_args),
        proc_args: Arc::clone(proc_args),
        stats: Arc::new(SslRuntimeStats::default()),
        histogram: Some(histogram),
        histogram_recorder,
    };

    super::run(target, proc_args).await
}
