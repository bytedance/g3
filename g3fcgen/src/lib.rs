/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, anyhow};
use tokio::runtime::Handle;
use tokio::time::Instant;

use g3_cert_agent::Request;
use g3_types::ext::DurationExt;

pub mod config;

mod build;

pub mod opts;
use opts::ProcArgs;

mod stat;

mod backend;
use backend::{BackendStats, OpensslBackend};

mod frontend;
use frontend::{Frontend, FrontendStats, GeneratedData};

struct BackendRequest {
    user_req: Request,
    peer: SocketAddr,
    recv_time: Instant,
}

struct BackendResponse {
    user_req: Request,
    generated: GeneratedData,
    peer: SocketAddr,
    recv_time: Instant,
}

impl BackendRequest {
    fn into_response(self, generated: GeneratedData) -> BackendResponse {
        BackendResponse {
            user_req: self.user_req,
            generated,
            peer: self.peer,
            recv_time: self.recv_time,
        }
    }
}

impl BackendResponse {
    fn duration(&self) -> u64 {
        self.recv_time.elapsed().as_nanos_u64()
    }
}

pub async fn run(proc_args: &ProcArgs) -> anyhow::Result<()> {
    let (req_sender, req_receiver) = flume::bounded::<BackendRequest>(1024);
    let (rsp_sender, rsp_receiver) = flume::bounded::<BackendResponse>(1024);

    let backend_config =
        config::get_backend_config().ok_or_else(|| anyhow!("no backend config available"))?;
    let backend_stats = Arc::new(BackendStats::default());

    let (duration_recorder, duration_stats) = backend_config.duration_stats.build_spawned(None);

    let workers = g3_daemon::runtime::worker::foreach(|h| {
        let backend = OpensslBackend::new(&backend_config, &backend_stats)
            .context(format!("failed to build backend for worker {}", h.id))?;
        backend.spawn(&h.handle, h.id, req_receiver.clone(), rsp_sender.clone());
        Ok::<(), anyhow::Error>(())
    })?;
    if workers < 1 {
        let backend = OpensslBackend::new(&backend_config, &backend_stats)
            .context("failed to build backend for main runtime")?;
        backend.spawn(&Handle::current(), 0, req_receiver, rsp_sender);
    } else {
        drop(rsp_sender);
    }

    let frontend = Frontend::new(proc_args.listen_config(), duration_recorder, rsp_receiver)?;

    if let Some(stats_config) = g3_daemon::stat::config::get_global_stat_config() {
        stat::spawn_working_thread(
            stats_config,
            backend_stats,
            duration_stats,
            frontend.stats(),
        )?;
    }

    frontend.run(req_sender).await
}
