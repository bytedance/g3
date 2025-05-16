/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use slog::Logger;

use g3_daemon::server::ClientConnectionInfo;
use g3_io_ext::IdleWheel;

use crate::config::server::rustls_proxy::RustlsProxyServerConfig;
use crate::module::stream::StreamServerStats;
use crate::serve::ServerQuitPolicy;

pub(crate) struct CommonTaskContext {
    pub server_config: Arc<RustlsProxyServerConfig>,
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
