/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use g3_daemon::control::quit::QuitAction;

use super::local::{DaemonController, UniqueController};

#[derive(Default)]
pub struct QuitActor {}

impl QuitAction for QuitActor {
    async fn do_release_controller(&self) {
        DaemonController::abort().await;
    }

    fn do_resume_controller(&self) -> anyhow::Result<()> {
        let daemon_ctl = DaemonController::start()?;
        tokio::spawn(async move {
            daemon_ctl.await;
        });
        Ok(())
    }

    async fn do_graceful_shutdown(&self) {
        UniqueController::abort_gracefully().await
    }

    async fn do_force_shutdown(&self) {
        UniqueController::abort_immediately().await
    }
}
