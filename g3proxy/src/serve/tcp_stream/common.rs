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

use std::net::SocketAddr;
use std::sync::Arc;

use slog::Logger;

use g3_daemon::server::ClientConnectionInfo;
use g3_types::net::OpensslClientConfig;

use super::stats::TcpStreamServerStats;
use crate::audit::AuditHandle;
use crate::config::server::tcp_stream::TcpStreamServerConfig;
use crate::escape::ArcEscaper;
use crate::serve::ServerQuitPolicy;

pub(super) struct CommonTaskContext {
    pub(super) server_config: Arc<TcpStreamServerConfig>,
    pub(super) server_stats: Arc<TcpStreamServerStats>,
    pub(super) server_quit_policy: Arc<ServerQuitPolicy>,
    pub(super) escaper: ArcEscaper,
    pub(super) audit_handle: Option<Arc<AuditHandle>>,
    pub(super) cc_info: ClientConnectionInfo,
    pub(super) tls_client_config: Option<Arc<OpensslClientConfig>>,
    pub(super) task_logger: Logger,
}

impl CommonTaskContext {
    #[inline]
    pub(crate) fn client_addr(&self) -> SocketAddr {
        self.cc_info.client_addr()
    }
}
