/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;

use arc_swap::ArcSwap;
use slog::Logger;

use g3_daemon::server::ClientConnectionInfo;
use g3_io_ext::IdleWheel;

use crate::backend::ArcBackend;
use crate::config::server::keyless_proxy::KeylessProxyServerConfig;
use crate::serve::ServerQuitPolicy;
use crate::serve::keyless_proxy::KeylessProxyServerStats;

#[derive(Clone)]
pub(crate) struct CommonTaskContext {
    pub server_config: Arc<KeylessProxyServerConfig>,
    pub server_stats: Arc<KeylessProxyServerStats>,
    pub server_quit_policy: Arc<ServerQuitPolicy>,
    pub idle_wheel: Arc<IdleWheel>,
    pub cc_info: ClientConnectionInfo,
    pub task_logger: Option<Logger>,
    pub backend_selector: Arc<ArcSwap<ArcBackend>>,
}

impl CommonTaskContext {
    pub(super) fn select_backend(&self) -> ArcBackend {
        self.backend_selector.load().as_ref().clone()
    }

    #[inline]
    pub(super) fn client_addr(&self) -> SocketAddr {
        self.cc_info.client_addr()
    }
}
