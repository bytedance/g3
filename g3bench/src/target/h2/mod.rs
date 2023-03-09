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
use crate::target::http::{HttpHistogram, HttpHistogramRecorder, HttpRuntimeStats};

mod opts;
use opts::BenchH2Args;

mod pool;
use pool::H2ConnectionPool;

mod task;
use task::H2TaskContext;

pub const COMMAND: &str = "h2";

struct H2Target {
    args: Arc<BenchH2Args>,
    proc_args: Arc<ProcArgs>,
    stats: Arc<HttpRuntimeStats>,
    histogram: Option<HttpHistogram>,
    pool: Option<Arc<H2ConnectionPool>>,
}

impl BenchTarget<HttpRuntimeStats, HttpHistogram, H2TaskContext> for H2Target {
    fn new_context(&self) -> anyhow::Result<H2TaskContext> {
        let histogram_recorder = self.histogram.as_ref().map(|h| h.recorder());
        H2TaskContext::new(
            &self.args,
            &self.proc_args,
            &self.stats,
            histogram_recorder,
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
    opts::add_h2_args(Command::new(COMMAND))
}

pub async fn run(proc_args: &Arc<ProcArgs>, cmd_args: &ArgMatches) -> anyhow::Result<()> {
    let mut h2_args = opts::parse_h2_args(cmd_args)?;
    h2_args.resolve_target_address(proc_args).await?;
    let h2_args = Arc::new(h2_args);

    let runtime_stats = Arc::new(HttpRuntimeStats::new(COMMAND));
    let histogram = Some(HttpHistogram::new());

    let pool = h2_args.pool_size.map(|s| {
        Arc::new(H2ConnectionPool::new(
            &h2_args,
            proc_args,
            s,
            &runtime_stats,
            histogram.as_ref(),
        ))
    });

    let target = H2Target {
        args: h2_args,
        proc_args: Arc::clone(proc_args),
        stats: runtime_stats,
        histogram,
        pool,
    };

    super::run(target, proc_args).await
}

struct H2PreRequest {
    method: Method,
    uri: Uri,
    host: HeaderValue,
    auth: Option<HeaderValue>,
}

impl H2PreRequest {
    fn build_request(&self) -> anyhow::Result<Request<()>> {
        let mut req = Request::builder()
            .version(Version::HTTP_2)
            .method(self.method.clone())
            .uri(self.uri.clone())
            .body(())
            .map_err(|e| anyhow!("failed to build request: {e:?}"))?;
        req.headers_mut()
            .insert(http::header::HOST, self.host.clone());
        if let Some(v) = &self.auth {
            req.headers_mut()
                .insert(http::header::AUTHORIZATION, v.clone());
        }
        Ok(req)
    }
}
