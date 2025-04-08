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
