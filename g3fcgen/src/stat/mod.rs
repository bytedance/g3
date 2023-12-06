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
use std::thread::JoinHandle;
use std::time::Instant;

use anyhow::anyhow;

use g3_histogram::HistogramStats;
use g3_statsd_client::StatsdClientConfig;

mod metrics;

use super::{BackendStats, FrontendStats};

pub(crate) fn spawn_working_thread(
    config: StatsdClientConfig,
    backend_stats: Arc<BackendStats>,
    backend_duration_stats: Arc<HistogramStats>,
    frontend_stats: Arc<FrontendStats>,
) -> anyhow::Result<JoinHandle<()>> {
    let mut client = config
        .build()
        .map_err(|e| anyhow!("failed to build statsd client: {e}"))?;

    let handle = std::thread::Builder::new()
        .name("stat-main".to_string())
        .spawn(move || loop {
            let instant_start = Instant::now();

            metrics::backend::emit_stats(&mut client, &backend_stats);
            metrics::backend::emit_duration_stats(&mut client, &backend_duration_stats);
            metrics::frontend::emit_stats(&mut client, &frontend_stats);

            client.flush_sink();

            g3_daemon::stat::emit::wait_duration(config.emit_duration, instant_start);
        })
        .map_err(|e| anyhow!("failed to spawn thread: {e:?}"))?;
    Ok(handle)
}
