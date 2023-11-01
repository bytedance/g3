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

use anyhow::{anyhow, Context};
use cadence::StatsdClient;
use log::warn;

use g3_histogram::HistogramStats;
use g3_statsd::client::StatsdClientConfig;

mod metrics;

use super::{BackendStats, FrontendStats};

fn build_statsd_client(config: &StatsdClientConfig) -> anyhow::Result<StatsdClient> {
    let builder = config.build().context("failed to build statsd client")?;

    let start_instant = Instant::now();
    let client = builder
        .with_error_handler(move |e| {
            static mut LAST_REPORT_TIME_SLICE: u64 = 0;
            let time_slice = start_instant.elapsed().as_secs().rotate_right(6); // every 64s
            unsafe {
                if LAST_REPORT_TIME_SLICE != time_slice {
                    warn!("sending metrics error: {e:?}");
                    LAST_REPORT_TIME_SLICE = time_slice;
                }
            }
        })
        .build();
    Ok(client)
}

pub(crate) fn spawn_working_thread(
    config: StatsdClientConfig,
    backend_stats: Arc<BackendStats>,
    backend_duration_stats: Arc<HistogramStats>,
    frontend_stats: Arc<FrontendStats>,
) -> anyhow::Result<JoinHandle<()>> {
    let client = build_statsd_client(&config).context("failed to build statsd client")?;

    let handle = std::thread::Builder::new()
        .name("stat-main".to_string())
        .spawn(move || loop {
            let instant_start = Instant::now();

            metrics::backend::emit_stats(&client, &backend_stats);
            metrics::backend::emit_duration_stats(&client, &backend_duration_stats);
            metrics::frontend::emit_stats(&client, &frontend_stats);

            client.flush_sink();

            g3_daemon::stat::emit::wait_duration(config.emit_duration, instant_start);
        })
        .map_err(|e| anyhow!("failed to spawn thread: {e:?}"))?;
    Ok(handle)
}
