/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use log::debug;

use g3_daemon::control::LocalController;

pub struct UniqueController {}
pub struct DaemonController {}

impl UniqueController {
    pub fn start() -> anyhow::Result<impl Future> {
        LocalController::start_unique(crate::build::PKG_NAME, crate::opts::daemon_group())
    }

    async fn abort(force: bool) {
        // make sure we always shut down protected io
        crate::control::disable_protected_io().await;

        debug!("stopping all servers");
        crate::serve::stop_all().await;
        debug!("stopped all servers");

        if !force {
            let delay = g3_daemon::runtime::config::get_task_wait_delay();
            debug!("will delay {delay:?} before waiting tasks");
            tokio::time::sleep(delay).await;
            let wait = g3_daemon::runtime::config::get_task_wait_timeout();
            let quit = g3_daemon::runtime::config::get_task_quit_timeout();
            crate::serve::wait_all_tasks(wait, quit, |name, left| {
                debug!("{left} tasks left on server {name}");
            })
            .await;
        }

        debug!("aborting unique controller");
        LocalController::abort_unique().await;
    }

    pub(super) async fn abort_immediately() {
        UniqueController::abort(true).await
    }

    pub(super) async fn abort_gracefully() {
        UniqueController::abort(false).await
    }
}

impl DaemonController {
    pub fn start() -> anyhow::Result<impl Future> {
        LocalController::start_daemon(crate::build::PKG_NAME, crate::opts::daemon_group())
    }

    pub(super) async fn abort() {
        // shutdown protected io before going to offline
        crate::control::disable_protected_io().await;

        debug!("aborting daemon controller");
        LocalController::abort_daemon().await;
    }
}
