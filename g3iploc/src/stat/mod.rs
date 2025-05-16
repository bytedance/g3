/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
