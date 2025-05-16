/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Instant;

use anyhow::{Context, anyhow};

use g3_statsd_client::{StatsdClient, StatsdClientConfig};

pub(crate) mod types;

mod metrics;
pub(crate) use metrics::user_site;

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
        .spawn(move || {
            loop {
                let instant_start = Instant::now();

                metrics::server::sync_stats();
                metrics::escaper::sync_stats();
                metrics::resolver::sync_stats();
                metrics::user::sync_stats();
                g3_daemon::log::metrics::sync_stats();

                metrics::server::emit_stats(&mut client);
                metrics::escaper::emit_stats(&mut client);
                metrics::resolver::emit_stats(&mut client);
                metrics::user::emit_stats(&mut client);
                g3_daemon::runtime::metrics::emit_stats(&mut client);
                g3_daemon::log::metrics::emit_stats(&mut client);

                client.flush_sink();

                if QUIT_STAT_THREAD.load(Ordering::Relaxed) {
                    break;
                }

                g3_daemon::stat::emit::wait_duration(emit_duration, instant_start);
            }
        })
        .map_err(|e| anyhow!("failed to spawn thread: {e:?}"))?;
    Ok(handle)
}

fn spawn_user_site_thread(config: &StatsdClientConfig) -> anyhow::Result<JoinHandle<()>> {
    let mut client = build_statsd_client(config)?;

    let emit_duration = config.emit_duration;
    let handle = std::thread::Builder::new()
        .name("stat-user-site".to_string())
        .spawn(move || {
            loop {
                let instant_start = Instant::now();

                user_site::sync_stats();
                user_site::emit_stats(&mut client);

                client.flush_sink();

                if QUIT_STAT_THREAD.load(Ordering::Relaxed) {
                    break;
                }

                g3_daemon::stat::emit::wait_duration(emit_duration, instant_start);
            }
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
