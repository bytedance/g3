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

use anyhow::anyhow;
use clap::{ArgMatches, Command};
use http::{HeaderValue, Method, Request, Uri, Version};

use super::{BenchTarget, BenchTaskContext, ProcArgs};
use crate::module::http::{HttpHistogram, HttpHistogramRecorder, HttpRuntimeStats};

mod opts;
use opts::BenchH3Args;

mod pool;
use pool::H3ConnectionPool;

mod task;
use task::H3TaskContext;

pub const COMMAND: &str = "h3";

struct H3Target {
    args: Arc<BenchH3Args>,
    proc_args: Arc<ProcArgs>,
    stats: Arc<HttpRuntimeStats>,
    histogram: Option<HttpHistogram>,
    histogram_recorder: HttpHistogramRecorder,
    pool: Option<Arc<H3ConnectionPool>>,
}

impl BenchTarget<HttpRuntimeStats, HttpHistogram, H3TaskContext> for H3Target {
    fn new_context(&self) -> anyhow::Result<H3TaskContext> {
        H3TaskContext::new(
            &self.args,
            &self.proc_args,
            &self.stats,
            self.histogram_recorder.clone(),
            self.pool.clone(),
        )
    }

    fn fetch_runtime_stats(&self) -> Arc<HttpRuntimeStats> {
        self.stats.clone()
    }

    fn take_histogram(&mut self) -> Option<HttpHistogram> {
        self.histogram.take()
    }

    fn notify_finish(&mut self) {
        self.pool = None;
    }
}

pub fn command() -> Command {
    opts::add_h3_args(Command::new(COMMAND))
}

pub async fn run(proc_args: &Arc<ProcArgs>, cmd_args: &ArgMatches) -> anyhow::Result<()> {
    let mut h3_args = opts::parse_h3_args(cmd_args)?;
    h3_args.resolve_target_address(proc_args).await?;
    let h3_args = Arc::new(h3_args);

    let runtime_stats = Arc::new(HttpRuntimeStats::new_udp(COMMAND));
    let (histogram, histogram_recorder) = HttpHistogram::new();

    let pool = h3_args.pool_size.map(|s| {
        Arc::new(H3ConnectionPool::new(
            &h3_args,
            proc_args,
            s,
            &runtime_stats,
            &histogram_recorder,
        ))
    });

    let target = H3Target {
        args: h3_args,
        proc_args: Arc::clone(proc_args),
        stats: runtime_stats,
        histogram: Some(histogram),
        histogram_recorder,
        pool,
    };

    super::run(target, proc_args).await
}

struct H3PreRequest {
    method: Method,
    uri: Uri,
    auth: Option<HeaderValue>,
}

impl H3PreRequest {
    fn build_request(&self) -> anyhow::Result<Request<()>> {
        let mut req = Request::builder()
            .version(Version::HTTP_3)
            .method(self.method.clone())
            .uri(self.uri.clone())
            .body(())
            .map_err(|e| anyhow!("failed to build request: {e:?}"))?;
        if let Some(v) = &self.auth {
            req.headers_mut()
                .insert(http::header::AUTHORIZATION, v.clone());
        }
        Ok(req)
    }
}
