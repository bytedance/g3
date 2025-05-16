/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use slog::Logger;

use g3_daemon::server::ClientConnectionInfo;
use g3_io_ext::IdleWheel;
use g3_types::net::OpensslClientConfig;

use super::stats::TcpStreamServerStats;
use crate::config::server::tcp_stream::TcpStreamServerConfig;
use crate::escape::ArcEscaper;
use crate::serve::ServerQuitPolicy;

pub(super) struct CommonTaskContext {
    pub(super) server_config: Arc<TcpStreamServerConfig>,
    pub(super) server_stats: Arc<TcpStreamServerStats>,
    pub(super) server_quit_policy: Arc<ServerQuitPolicy>,
    pub(super) idle_wheel: Arc<IdleWheel>,
    pub(super) escaper: ArcEscaper,
    pub(super) cc_info: ClientConnectionInfo,
    pub(super) tls_client_config: Option<Arc<OpensslClientConfig>>,
    pub(super) task_logger: Option<Logger>,
}

impl CommonTaskContext {
    pub(super) fn log_flush_interval(&self) -> Option<Duration> {
        self.task_logger.as_ref()?;
        self.server_config.task_log_flush_interval
    }
}
