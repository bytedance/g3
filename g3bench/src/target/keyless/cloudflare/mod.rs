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

use std::process::ExitCode;
use std::sync::Arc;

use clap::{ArgMatches, Command};

use super::{BenchTarget, BenchTaskContext, ProcArgs};

mod opts;
use opts::KeylessCloudflareArgs;

mod stats;
use stats::{KeylessHistogram, KeylessHistogramRecorder, KeylessRuntimeStats};

mod task;
use task::KeylessCloudflareTaskContext;

mod message;
use message::{
    KeylessLocalError, KeylessRequest, KeylessRequestBuilder, KeylessResponse, KeylessResponseError,
};

mod connection;
use connection::{MultiplexTransfer, SimplexTransfer};

mod pool;
use pool::KeylessConnectionPool;

pub(super) const COMMAND: &str = "cloudflare";

struct KeylessCloudflareTarget {
    args: Arc<KeylessCloudflareArgs>,
    proc_args: Arc<ProcArgs>,
    stats: Arc<KeylessRuntimeStats>,
    histogram: Option<KeylessHistogram>,
    histogram_recorder: KeylessHistogramRecorder,
    pool: Option<Arc<KeylessConnectionPool>>,
}

impl BenchTarget<KeylessRuntimeStats, KeylessHistogram, KeylessCloudflareTaskContext>
    for KeylessCloudflareTarget
{
    fn new_context(&self) -> anyhow::Result<KeylessCloudflareTaskContext> {
        KeylessCloudflareTaskContext::new(
            &self.args,
            &self.proc_args,
            &self.stats,
            self.histogram_recorder.clone(),
            self.pool.clone(),
        )
    }

    fn fetch_runtime_stats(&self) -> Arc<KeylessRuntimeStats> {
        self.stats.clone()
    }

    fn take_histogram(&mut self) -> Option<KeylessHistogram> {
        self.histogram.take()
    }

    fn notify_finish(&mut self) {
        self.pool = None;
    }
}

pub(super) fn command() -> Command {
    opts::add_cloudflare_args(
        Command::new(COMMAND).about("Use keyless server that speaks cloudflare protocol"),
    )
}

pub(super) async fn run(
    proc_args: &Arc<ProcArgs>,
    cmd_args: &ArgMatches,
) -> anyhow::Result<ExitCode> {
    let mut cf_args = opts::parse_cloudflare_args(cmd_args)?;
    cf_args.resolve_target_address(proc_args).await?;

    let cf_args = Arc::new(cf_args);

    let runtime_stats = Arc::new(KeylessRuntimeStats::default());
    let (histogram, histogram_recorder) = KeylessHistogram::new();

    let pool = cf_args.pool_size.map(|s| {
        Arc::new(KeylessConnectionPool::new(
            &cf_args,
            proc_args,
            s,
            &runtime_stats,
            &histogram_recorder,
        ))
    });

    let target = KeylessCloudflareTarget {
        args: cf_args,
        proc_args: Arc::clone(proc_args),
        stats: runtime_stats,
        histogram: Some(histogram),
        histogram_recorder,
        pool,
    };

    crate::target::run(target, proc_args).await
}
