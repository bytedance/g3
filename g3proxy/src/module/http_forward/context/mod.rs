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

use std::time::Duration;

use async_trait::async_trait;

use g3_types::net::{HttpForwardCapability, OpensslTlsClientConfig, UpstreamAddr};

use super::{ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, HttpConnectionEofPoller};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

mod direct;
pub(crate) use direct::DirectHttpForwardContext;

mod proxy;
pub(crate) use proxy::ProxyHttpForwardContext;

mod route;
pub(crate) use route::RouteHttpForwardContext;

mod failover;
pub(crate) use failover::FailoverHttpForwardContext;

pub(crate) type BoxHttpForwardContext = Box<dyn HttpForwardContext + Send>;

#[async_trait]
pub(crate) trait HttpForwardContext {
    async fn check_in_final_escaper<'a>(
        &'a mut self,
        task_notes: &'a ServerTaskNotes,
        upstream: &'a UpstreamAddr,
    ) -> HttpForwardCapability;

    fn prepare_connection(&mut self, ups: &UpstreamAddr, is_tls: bool);
    async fn get_alive_connection<'a>(
        &'a mut self,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
        idle_expire: Duration,
    ) -> Option<BoxHttpForwardConnection>;
    async fn make_new_http_connection<'a>(
        &'a mut self,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError>;
    async fn make_new_https_connection<'a>(
        &'a mut self,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
        tls_config: &'a OpensslTlsClientConfig,
        tls_name: &'a str,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError>;
    fn save_alive_connection(&mut self, c: BoxHttpForwardConnection);
    fn fetch_tcp_notes(&self, tcp_notes: &mut TcpConnectTaskNotes);
}
