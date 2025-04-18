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

use log::debug;

use g3_daemon::control::LocalController;

pub struct UniqueController {
    inner: LocalController,
}
pub struct DaemonController {}

impl UniqueController {
    pub fn create() -> anyhow::Result<Self> {
        let controller =
            LocalController::create_unique(crate::build::PKG_NAME, crate::opts::daemon_group())?;
        Ok(UniqueController { inner: controller })
    }

    pub fn start(self) -> anyhow::Result<impl Future> {
        self.inner.start_as_unique()
    }

    #[inline]
    pub fn listen_path(&self) -> String {
        self.inner.listen_path()
    }

    async fn abort(force: bool) {
        // make sure we always shut down protected io
        // crate::control::disable_protected_io().await;

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
        // crate::control::disable_protected_io().await;

        debug!("aborting daemon controller");
        LocalController::abort_daemon().await;
    }
}
