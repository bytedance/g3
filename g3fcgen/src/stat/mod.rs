/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
        .spawn(move || {
            loop {
                let instant_start = Instant::now();

                metrics::backend::emit_stats(&mut client, &backend_stats);
                metrics::backend::emit_duration_stats(&mut client, &backend_duration_stats);
                metrics::frontend::emit_stats(&mut client, &frontend_stats);
                g3_daemon::runtime::metrics::emit_stats(&mut client);

                client.flush_sink();

                g3_daemon::stat::emit::wait_duration(config.emit_duration, instant_start);
            }
        })
        .map_err(|e| anyhow!("failed to spawn thread: {e:?}"))?;
    Ok(handle)
}
