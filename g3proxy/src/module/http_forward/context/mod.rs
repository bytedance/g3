/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use async_trait::async_trait;

use g3_types::net::{HttpForwardCapability, UpstreamAddr};

use super::{ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, HttpConnectionEofPoller};
use crate::audit::AuditContext;
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf,
};
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
    async fn check_in_final_escaper(
        &mut self,
        task_notes: &ServerTaskNotes,
        upstream: &UpstreamAddr,
        audit_ctx: &mut AuditContext,
    ) -> HttpForwardCapability;

    fn prepare_connection(&mut self, ups: &UpstreamAddr, is_tls: bool);
    async fn get_alive_connection(
        &mut self,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
        idle_expire: Duration,
    ) -> Option<BoxHttpForwardConnection>;
    async fn make_new_http_connection(
        &mut self,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError>;
    async fn make_new_https_connection(
        &mut self,
        task_conf: &TlsConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError>;
    fn save_alive_connection(&mut self, c: BoxHttpForwardConnection);
    fn fetch_tcp_notes(&self, tcp_notes: &mut TcpConnectTaskNotes);
}
