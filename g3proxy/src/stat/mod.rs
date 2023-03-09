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
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context};
use cadence::StatsdClient;
use log::warn;

use g3_statsd::client::StatsdClientConfig;

pub(crate) mod types;

mod metric;
pub(crate) use metric::user_site;

static QUIT_STAT_THREAD: AtomicBool = AtomicBool::new(false);

fn build_statsd_client(config: &StatsdClientConfig) -> anyhow::Result<StatsdClient> {
    let builder = config.build().context("failed to build statsd client")?;

    let start_instant = Instant::now();
    let client = builder
        .with_tag(
            g3_daemon::metric::TAG_KEY_DAEMON_GROUP,
            crate::config::daemon_group_name(),
        )
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

fn wait_duration(emit_duration: Duration, instant_start: Instant) {
    let instant_now = Instant::now();
    if let Some(instant_next) = instant_start.checked_add(emit_duration) {
        // re-calculate the duration
        if let Some(dur) = instant_next.checked_duration_since(instant_now) {
            std::thread::sleep(dur);
        }
    } else {
        std::thread::sleep(emit_duration);
    }
}

fn spawn_main_thread(config: &StatsdClientConfig) -> anyhow::Result<JoinHandle<()>> {
    let client = build_statsd_client(config).context("failed to build statsd client")?;

    let emit_duration = config.emit_duration;
    let handle = std::thread::Builder::new()
        .name("stat-main".to_string())
        .spawn(move || loop {
            let instant_start = Instant::now();

            metric::server::sync_stats();
            metric::escaper::sync_stats();
            metric::resolver::sync_stats();
            metric::user::sync_stats();
            g3_daemon::log::metric::sync_stats();

            metric::server::emit_stats(&client);
            metric::escaper::emit_stats(&client);
            metric::resolver::emit_stats(&client);
            metric::user::emit_stats(&client);
            g3_daemon::log::metric::emit_stats(&client);

            client.flush_sink();

            if QUIT_STAT_THREAD.load(Ordering::Relaxed) {
                break;
            }

            wait_duration(emit_duration, instant_start);
        })
        .map_err(|e| anyhow!("failed to spawn thread: {e:?}"))?;
    Ok(handle)
}

fn spawn_user_site_thread(config: &StatsdClientConfig) -> anyhow::Result<JoinHandle<()>> {
    let client = build_statsd_client(config).context("failed to build statsd client")?;

    let emit_duration = config.emit_duration;
    let handle = std::thread::Builder::new()
        .name("stat-user-site".to_string())
        .spawn(move || loop {
            let instant_start = Instant::now();

            user_site::sync_stats();
            user_site::emit_stats(&client);

            client.flush_sink();

            if QUIT_STAT_THREAD.load(Ordering::Relaxed) {
                break;
            }

            wait_duration(emit_duration, instant_start);
        })
        .map_err(|e| anyhow!("failed to spawn thread: {e:?}"))?;
    Ok(handle)
}

pub fn spawn_working_threads(config: StatsdClientConfig) -> anyhow::Result<Vec<JoinHandle<()>>> {
    let mut handlers = Vec::with_capacity(2);
    let main_handle = spawn_main_thread(&config).context("failed to spawn main stats thread")?;
    handlers.push(main_handle);
    let user_site_handle =
        spawn_user_site_thread(&config).context("failed to spawn user site stats thread")?;
    handlers.push(user_site_handle);
    Ok(handlers)
}

pub fn stop_working_threads() {
    QUIT_STAT_THREAD.store(true, Ordering::Relaxed);
}
