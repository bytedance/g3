/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use slog::Logger;
use tokio::time::Instant;

use g3_daemon::server::ClientConnectionInfo;
use g3_io_ext::{IdleWheel, OptionalInterval};

use super::{HttpRProxyServerConfig, HttpRProxyServerStats};
use crate::escape::ArcEscaper;
use crate::serve::ServerQuitPolicy;

#[derive(Clone)]
pub(crate) struct CommonTaskContext {
    pub(crate) server_config: Arc<HttpRProxyServerConfig>,
    pub(crate) server_stats: Arc<HttpRProxyServerStats>,
    pub(crate) server_quit_policy: Arc<ServerQuitPolicy>,
    pub(crate) idle_wheel: Arc<IdleWheel>,
    pub(crate) escaper: ArcEscaper,
    pub(crate) cc_info: ClientConnectionInfo,
    pub(crate) task_logger: Option<Logger>,
}

impl CommonTaskContext {
    #[inline]
    pub(crate) fn client_addr(&self) -> SocketAddr {
        self.cc_info.client_addr()
    }

    #[inline]
    pub(crate) fn client_ip(&self) -> IpAddr {
        self.cc_info.client_ip()
    }

    #[inline]
    pub(crate) fn server_addr(&self) -> SocketAddr {
        self.cc_info.server_addr()
    }

    pub(super) fn log_flush_interval(&self) -> Option<Duration> {
        self.task_logger.as_ref()?;
        self.server_config.task_log_flush_interval
    }

    pub(super) fn get_log_interval(&self) -> OptionalInterval {
        self.log_flush_interval()
            .map(|log_interval| {
                let log_interval =
                    tokio::time::interval_at(Instant::now() + log_interval, log_interval);
                OptionalInterval::with(log_interval)
            })
            .unwrap_or_default()
    }
}
