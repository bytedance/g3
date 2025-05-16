/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use log::debug;

use g3_daemon::control::LocalController;

pub struct UniqueController {}
pub struct DaemonController {}

impl UniqueController {
    pub fn start() -> anyhow::Result<impl Future> {
        LocalController::start_unique(crate::build::PKG_NAME, crate::opts::daemon_group())
    }

    pub(super) async fn abort_immediately() {
        debug!("aborting unique controller");
        LocalController::abort_unique().await;
    }

    pub(super) async fn abort_gracefully() {
        debug!("stopping all importers");
        crate::import::stop_all().await;
        debug!("stopped all importers");

        // TODO flush and stop all exporters

        UniqueController::abort_immediately().await
    }
}

impl DaemonController {
    pub fn start() -> anyhow::Result<impl Future> {
        LocalController::start_daemon(crate::build::PKG_NAME, crate::opts::daemon_group())
    }

    pub(super) async fn abort() {
        debug!("aborting daemon controller");
        LocalController::abort_daemon().await;
    }
}
