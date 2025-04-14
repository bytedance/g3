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

use std::sync::Arc;
use std::time::Duration;

use slog::Logger;

use g3_daemon::server::ClientConnectionInfo;
use g3_io_ext::IdleWheel;

use crate::config::server::openssl_proxy::OpensslProxyServerConfig;
use crate::module::stream::StreamServerStats;
use crate::serve::ServerQuitPolicy;

pub(crate) struct CommonTaskContext {
    pub server_config: Arc<OpensslProxyServerConfig>,
    pub server_stats: Arc<StreamServerStats>,
    pub server_quit_policy: Arc<ServerQuitPolicy>,
    pub idle_wheel: Arc<IdleWheel>,
    pub cc_info: ClientConnectionInfo,
    pub task_logger: Option<Logger>,
}

impl CommonTaskContext {
    pub(super) fn log_flush_interval(&self) -> Option<Duration> {
        self.task_logger.as_ref()?;
        self.server_config.task_log_flush_interval
    }
}
