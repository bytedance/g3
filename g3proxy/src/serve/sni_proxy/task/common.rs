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
use std::os::unix::prelude::*;
use std::sync::Arc;

use slog::Logger;

use g3_dpi::ProtocolPortMap;

use crate::audit::AuditHandle;
use crate::config::server::sni_proxy::SniProxyServerConfig;
use crate::escape::ArcEscaper;
use crate::serve::tcp_stream::TcpStreamServerStats;
use crate::serve::ServerQuitPolicy;

pub(crate) struct CommonTaskContext {
    pub server_config: Arc<SniProxyServerConfig>,
    pub server_stats: Arc<TcpStreamServerStats>,
    pub server_quit_policy: Arc<ServerQuitPolicy>,
    pub escaper: ArcEscaper,
    pub audit_handle: Option<Arc<AuditHandle>>,
    pub server_addr: SocketAddr,
    pub client_addr: SocketAddr,
    pub task_logger: Logger,
    pub worker_id: Option<usize>,

    pub server_tcp_portmap: Arc<ProtocolPortMap>,
    pub client_tcp_portmap: Arc<ProtocolPortMap>,

    pub tcp_client_socket: RawFd,
}
