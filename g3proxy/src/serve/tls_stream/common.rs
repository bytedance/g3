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

use slog::Logger;

use g3_daemon::server::ClientConnectionInfo;
use g3_io_ext::IdleWheel;
use g3_types::net::OpensslClientConfig;

use crate::config::server::tls_stream::TlsStreamServerConfig;
use crate::escape::ArcEscaper;
use crate::serve::tcp_stream::TcpStreamServerStats;
use crate::serve::ServerQuitPolicy;

pub(super) struct CommonTaskContext {
    pub(super) server_config: Arc<TlsStreamServerConfig>,
    pub(super) server_stats: Arc<TcpStreamServerStats>,
    pub(super) server_quit_policy: Arc<ServerQuitPolicy>,
    pub(super) idle_wheel: Arc<IdleWheel>,
    pub(super) escaper: ArcEscaper,
    pub(super) cc_info: ClientConnectionInfo,
    pub(super) tls_client_config: Option<Arc<OpensslClientConfig>>,
    pub(super) task_logger: Logger,
}
