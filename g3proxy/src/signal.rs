/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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

    if let Err(e) = crate::resolve::spawn_all().await {
        error!("failed to reload all resolvers: {e:?}");
    }
    if let Err(e) = crate::escape::load_all().await {
        error!("failed to reload all escapers: {e:?}");
    }
    if let Err(e) = crate::auth::load_all().await {
        error!("failed to reload all user groups: {e:?}");
    }
    if let Err(e) = crate::audit::load_all().await {
        error!("failed to reload all auditors: {e:?}");
    }
    if let Err(e) = crate::serve::spawn_all().await {
        error!("failed to reload all servers: {e:?}");
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
        g3_daemon::control::quit::start_graceful_shutdown().await
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
