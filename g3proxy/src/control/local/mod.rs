/*
 * Copyright 2023 ByteDance and/or its affiliates.
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

use std::future::Future;

use log::debug;

use g3_daemon::control::LocalController;

pub struct UniqueController {}
pub struct DaemonController {}

impl UniqueController {
    pub fn start() -> anyhow::Result<impl Future> {
        LocalController::start_unique(
            crate::opts::control_dir(),
            crate::config::daemon_group_name(),
        )
    }

    async fn abort(force: bool) {
        // make sure we always shutdown protected io
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
        LocalController::abort_unique();
    }

    pub async fn abort_immediately() {
        UniqueController::abort(true).await
    }

    pub async fn abort_gracefully() {
        UniqueController::abort(false).await
    }
}

impl DaemonController {
    pub fn start() -> anyhow::Result<impl Future> {
        LocalController::start_daemon(
            crate::opts::control_dir(),
            crate::config::daemon_group_name(),
        )
    }

    pub async fn abort() {
        // shutdown protected io before going to offline
        crate::control::disable_protected_io().await;

        debug!("aborting daemon controller");
        LocalController::abort_daemon();

        tokio::spawn(async {
            let delay = g3_daemon::runtime::config::get_server_offline_delay();
            if !delay.is_zero() {
                debug!("will stop all servers after {delay:?}");
                tokio::time::sleep(delay).await;
            }

            UniqueController::abort_gracefully().await
        });
    }
}
