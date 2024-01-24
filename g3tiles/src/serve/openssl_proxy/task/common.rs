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

use openssl::ex_data::Index;
use openssl::ssl::{Ssl, SslContext, SslVersion};
use slog::Logger;

use g3_daemon::server::ClientConnectionInfo;
use g3_types::limit::GaugeSemaphorePermit;

use crate::config::server::openssl_proxy::OpensslProxyServerConfig;
use crate::serve::openssl_proxy::host::OpensslHost;
use crate::serve::openssl_proxy::OpensslProxyServerStats;
use crate::serve::ServerQuitPolicy;

pub(crate) struct CommonTaskContext {
    pub server_config: Arc<OpensslProxyServerConfig>,
    pub server_stats: Arc<OpensslProxyServerStats>,
    pub server_quit_policy: Arc<ServerQuitPolicy>,
    pub cc_info: ClientConnectionInfo,
    pub task_logger: Logger,

    pub client_hello_version_index: Index<Ssl, SslVersion>,
    pub host_index: Index<Ssl, Option<Arc<OpensslHost>>>,
    pub alive_permit_index: Index<Ssl, Option<GaugeSemaphorePermit>>,
    pub ssl_context: Arc<SslContext>,
    #[cfg(feature = "vendored-tongsuo")]
    pub tlcp_context: Arc<SslContext>,
}

impl CommonTaskContext {
    #[inline]
    pub(super) fn client_addr(&self) -> SocketAddr {
        self.cc_info.client_addr()
    }
}
