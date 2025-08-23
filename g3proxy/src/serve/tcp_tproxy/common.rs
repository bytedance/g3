/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use slog::Logger;

use g3_daemon::server::ClientConnectionInfo;
use g3_io_ext::IdleWheel;

use crate::config::server::tcp_tproxy::TcpTProxyServerConfig;
use crate::escape::ArcEscaper;
use crate::serve::ServerQuitPolicy;
use crate::serve::tcp_stream::TcpStreamServerStats;

pub(super) struct CommonTaskContext {
    pub(super) server_config: Arc<TcpTProxyServerConfig>,
    pub(super) server_stats: Arc<TcpStreamServerStats>,
    pub(super) server_quit_policy: Arc<ServerQuitPolicy>,
    pub(super) idle_wheel: Arc<IdleWheel>,
    pub(super) escaper: ArcEscaper,
    pub(super) cc_info: ClientConnectionInfo,
    pub(super) task_logger: Option<Logger>,
}

impl CommonTaskContext {
    #[inline]
    pub(super) fn target_addr(&self) -> SocketAddr {
        self.cc_info.server_addr()
    }

    pub(super) fn log_flush_interval(&self) -> Option<Duration> {
        self.task_logger.as_ref()?;
        self.server_config.task_log_flush_interval
    }
}
