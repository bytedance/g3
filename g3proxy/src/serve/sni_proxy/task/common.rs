/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use slog::Logger;

use g3_daemon::server::ClientConnectionInfo;
use g3_dpi::ProtocolPortMap;
use g3_io_ext::IdleWheel;

use crate::auth::FactsUserGroup;
use crate::config::server::sni_proxy::SniProxyServerConfig;
use crate::escape::ArcEscaper;
use crate::serve::ServerQuitPolicy;
use crate::serve::tcp_stream::TcpStreamServerStats;

pub(crate) struct CommonTaskContext {
    pub(crate) server_config: Arc<SniProxyServerConfig>,
    pub(crate) server_stats: Arc<TcpStreamServerStats>,
    pub(crate) server_quit_policy: Arc<ServerQuitPolicy>,
    pub(crate) idle_wheel: Arc<IdleWheel>,
    pub(crate) escaper: ArcEscaper,
    pub(crate) user_group: Option<Arc<FactsUserGroup>>,
    pub(crate) cc_info: ClientConnectionInfo,
    pub(crate) task_logger: Option<Logger>,

    pub(crate) server_tcp_portmap: Arc<ProtocolPortMap>,
    pub(crate) client_tcp_portmap: Arc<ProtocolPortMap>,
}

impl CommonTaskContext {
    #[inline]
    pub(crate) fn client_addr(&self) -> SocketAddr {
        self.cc_info.client_addr()
    }

    #[inline]
    pub(crate) fn server_port(&self) -> u16 {
        self.cc_info.server_addr().port()
    }

    pub(super) fn log_flush_interval(&self) -> Option<Duration> {
        self.task_logger.as_ref()?;
        self.server_config.task_log_flush_interval
    }
}
