/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use log::{error, info, warn};
use tokio::sync::Mutex;

use g3_daemon::signal::AsyncSignalAction;

static RELOAD_MUTEX: Mutex<()> = Mutex::const_new(());

async fn do_reload() {
    let _guard = RELOAD_MUTEX.lock().await;
    info!("reloading config");

    if let Err(e) = crate::config::reload().await {
        warn!("error reloading config: {e:?}");
        warn!("reload aborted");
    }

    if let Err(e) = crate::collect::spawn_all().await {
        error!("failed to reload all collectors: {e}");
    }
    if let Err(e) = crate::input::spawn_all().await {
        error!("failed to reload all inputs: {e}");
    }

    info!("reload finished");
}

#[derive(Clone, Copy)]
struct QuitAction {}

impl AsyncSignalAction for QuitAction {
    async fn run(&self) {
        g3_daemon::control::quit::trigger_force_shutdown();
    }
}

#[allow(unused)]
#[derive(Clone, Copy)]
struct OfflineAction {}

impl AsyncSignalAction for OfflineAction {
    async fn run(&self) {
        g3_daemon::control::quit::start_graceful_shutdown().await;
    }
}

#[allow(unused)]
#[derive(Clone, Copy)]
struct ReloadAction {}

impl AsyncSignalAction for ReloadAction {
    async fn run(&self) {
        do_reload().await
    }
}

pub fn register() -> anyhow::Result<()> {
    #[cfg(unix)]
    g3_daemon::signal::register_reload(ReloadAction {})?;
    #[cfg(unix)]
    g3_daemon::signal::register_offline(OfflineAction {})?;
    g3_daemon::signal::register_quit(QuitAction {})
}
