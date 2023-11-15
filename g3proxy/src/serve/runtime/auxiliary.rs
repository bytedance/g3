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

use quinn::Connecting;
use tokio::net::TcpStream;
use tokio::runtime::Handle;

use g3_daemon::server::ClientConnectionInfo;
use g3_types::metrics::MetricsName;
use g3_types::net::UdpListenConfig;

use crate::serve::{ArcServer, ServerRunContext};

pub(crate) trait AuxiliaryServerConfig {
    fn next_server(&self) -> &MetricsName;
    fn run_tcp_task(
        &self,
        _rt_handle: Handle,
        _next_server: ArcServer,
        _stream: TcpStream,
        _cc_info: ClientConnectionInfo,
        _ctx: ServerRunContext,
    ) {
    }

    fn run_quic_task(
        &self,
        _rt_handle: Handle,
        _next_server: ArcServer,
        _connecting: Connecting,
        _cc_info: ClientConnectionInfo,
        _ctx: ServerRunContext,
    ) {
    }

    fn take_udp_listen_config(&mut self) -> Option<UdpListenConfig> {
        None
    }

    fn take_quinn_config(&mut self) -> Option<quinn::ServerConfig> {
        None
    }
}
