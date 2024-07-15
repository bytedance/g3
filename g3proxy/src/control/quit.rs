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

use g3_daemon::control::quit::QuitAction;

use super::local::{DaemonController, UniqueController};

pub struct QuitActor {}

impl QuitActor {
    pub fn spawn_run() {
        let actor = QuitActor {};
        tokio::spawn(actor.into_running(g3_daemon::runtime::config::get_server_offline_delay()));
    }
}

impl QuitAction for QuitActor {
    async fn release_controller(&self) {
        DaemonController::abort().await;
    }

    fn resume_controller(&self) -> anyhow::Result<()> {
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
