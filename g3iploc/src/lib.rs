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

pub mod config;

mod build;

pub mod opts;
use opts::ProcArgs;

mod stat;

mod frontend;
use frontend::{FrontendStats, UdpDgramFrontend};

pub async fn run(proc_args: &ProcArgs) -> anyhow::Result<()> {
    let frontend_stats = Arc::new(FrontendStats::default());
    if let Some(stats_config) = g3_daemon::stat::config::get_global_stat_config() {
        stat::spawn_working_thread(stats_config, frontend_stats.clone())?;
    }

    let udp_listen_addr = proc_args.udp_listen_addr();

    let frontend = UdpDgramFrontend::new(udp_listen_addr, frontend_stats).await?;
    frontend.into_running().await;
    Ok(())
}
