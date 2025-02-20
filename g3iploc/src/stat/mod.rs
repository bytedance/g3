/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use g3_statsd_client::StatsdClientConfig;

use super::FrontendStats;

mod metrics;

pub(crate) fn spawn_working_thread(
    config: StatsdClientConfig,
    frontend_stats: Arc<FrontendStats>,
) -> anyhow::Result<JoinHandle<()>> {
    let mut client = config
        .build()
        .map_err(|e| anyhow!("failed to build statsd client: {e}"))?;

    let handle = std::thread::Builder::new()
        .name("stat-main".to_string())
        .spawn(move || {
            loop {
                let instant_start = Instant::now();

                metrics::frontend::emit_stats(&mut client, &frontend_stats);

                client.flush_sink();

                g3_daemon::stat::emit::wait_duration(config.emit_duration, instant_start);
            }
        })
        .map_err(|e| anyhow!("failed to spawn thread: {e:?}"))?;
    Ok(handle)
}
