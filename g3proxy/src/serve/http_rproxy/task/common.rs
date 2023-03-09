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

use super::{HttpRProxyServerConfig, HttpRProxyServerStats};
use crate::escape::ArcEscaper;
use crate::serve::ServerQuitPolicy;

#[derive(Clone)]
pub(crate) struct CommonTaskContext {
    pub(crate) server_config: Arc<HttpRProxyServerConfig>,
    pub(crate) server_stats: Arc<HttpRProxyServerStats>,
    pub(crate) server_quit_policy: Arc<ServerQuitPolicy>,
    pub(crate) escaper: ArcEscaper,
    pub(crate) tcp_server_addr: SocketAddr,
    pub(crate) tcp_client_addr: SocketAddr,
    pub(crate) task_logger: Logger,
    pub(crate) worker_id: Option<usize>,

    pub(crate) tcp_client_socket: RawFd,
}
