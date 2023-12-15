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

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Instant;

use anyhow::{anyhow, Context};

use g3_statsd_client::{StatsdClient, StatsdClientConfig};

mod metrics;

static QUIT_STAT_THREAD: AtomicBool = AtomicBool::new(false);

fn build_statsd_client(config: &StatsdClientConfig) -> anyhow::Result<StatsdClient> {
    let client = config
        .build()
        .map_err(|e| anyhow!("failed to build statsd client: {e}"))?;

    Ok(client.with_tag(
        g3_daemon::metrics::TAG_KEY_DAEMON_GROUP,
        crate::opts::daemon_group(),
    ))
}

fn spawn_main_thread(config: &StatsdClientConfig) -> anyhow::Result<JoinHandle<()>> {
    let mut client = build_statsd_client(config)?;

    let emit_duration = config.emit_duration;
    let handle = std::thread::Builder::new()
        .name("stat-main".to_string())
        .spawn(move || loop {
            let instant_start = Instant::now();

            metrics::server::sync_stats();
            g3_daemon::log::metrics::sync_stats();

            metrics::server::emit_stats(&mut client);
            g3_daemon::log::metrics::emit_stats(&mut client);

            client.flush_sink();

            if QUIT_STAT_THREAD.load(Ordering::Relaxed) {
                break;
            }

            g3_daemon::stat::emit::wait_duration(emit_duration, instant_start);
        })
        .map_err(|e| anyhow!("failed to spawn thread: {e:?}"))?;
    Ok(handle)
}

pub fn spawn_working_threads(config: StatsdClientConfig) -> anyhow::Result<Vec<JoinHandle<()>>> {
    let mut handlers = Vec::with_capacity(2);
    let main_handle = spawn_main_thread(&config).context("failed to spawn main stats thread")?;
    handlers.push(main_handle);
    Ok(handlers)
}

pub fn stop_working_threads() {
    QUIT_STAT_THREAD.store(true, Ordering::Relaxed);
}
