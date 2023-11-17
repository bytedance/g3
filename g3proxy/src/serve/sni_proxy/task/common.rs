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
use g3_dpi::ProtocolPortMap;

use crate::audit::AuditHandle;
use crate::config::server::sni_proxy::SniProxyServerConfig;
use crate::escape::ArcEscaper;
use crate::serve::tcp_stream::TcpStreamServerStats;
use crate::serve::ServerQuitPolicy;

pub(crate) struct CommonTaskContext {
    pub(crate) server_config: Arc<SniProxyServerConfig>,
    pub(crate) server_stats: Arc<TcpStreamServerStats>,
    pub(crate) server_quit_policy: Arc<ServerQuitPolicy>,
    pub(crate) escaper: ArcEscaper,
    pub(crate) audit_handle: Option<Arc<AuditHandle>>,
    pub(crate) cc_info: ClientConnectionInfo,
    pub(crate) task_logger: Logger,

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
}
