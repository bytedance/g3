/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::metrics::NodeName;
use g3_types::net::UpstreamAddr;

use super::{ArcEscaper, Escaper, EscaperInternal, EscaperRegistry, RouteEscaperStats};
use crate::audit::AuditContext;
use crate::config::escaper::route_query::RouteQueryEscaperConfig;
use crate::config::escaper::{AnyEscaperConfig, EscaperConfig};
use crate::module::ftp_over_http::{
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpConnectContext,
    BoxFtpRemoteConnection,
};
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, BoxHttpForwardContext,
    RouteHttpForwardContext,
};
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectResult, TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectResult, UdpConnectTaskConf, UdpConnectTaskNotes,
};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupResult, UdpRelayTaskConf, UdpRelayTaskNotes,
};
use crate::serve::ServerTaskNotes;

mod cache;
mod query;

use cache::CacheHandle;

pub(super) struct RouteQueryEscaper {
    config: Arc<RouteQueryEscaperConfig>,
    stats: Arc<RouteEscaperStats>,
    query_nodes: BTreeMap<NodeName, ArcEscaper>,
    fallback_node: ArcEscaper,
    cache_handle: CacheHandle,
}

impl RouteQueryEscaper {
    fn new_obj<F>(
        config: Arc<RouteQueryEscaperConfig>,
        stats: Arc<RouteEscaperStats>,
        mut fetch_escaper: F,
    ) -> anyhow::Result<ArcEscaper>
    where
        F: FnMut(&NodeName) -> ArcEscaper,
    {
        let mut query_nodes = BTreeMap::new();
        for name in &config.query_allowed_nodes {
            let escaper = fetch_escaper(name);
            query_nodes.insert(name.clone(), escaper);
        }

        let fallback_node = fetch_escaper(&config.fallback_node);

        let cache_handle = cache::spawn(&config)?;

        let escaper = RouteQueryEscaper {
            config,
            stats,
            query_nodes,
            fallback_node,
            cache_handle,
        };

        Ok(Arc::new(escaper))
    }

    pub(super) fn prepare_initial(config: RouteQueryEscaperConfig) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::new(RouteEscaperStats::new(config.name()));
        RouteQueryEscaper::new_obj(
            Arc::new(config),
            stats,
            super::registry::get_or_insert_default,
        )
    }

    fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<RouteEscaperStats>,
        registry: &mut EscaperRegistry,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::RouteQuery(config) = config {
            RouteQueryEscaper::new_obj(Arc::new(config), stats, |name| {
                registry.get_or_insert_default(name)
            })
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    async fn select_query(
        &self,
        task_notes: &ServerTaskNotes,
        upstream: &UpstreamAddr,
    ) -> Option<&ArcEscaper> {
        self.cache_handle
            .select(&self.config, task_notes, upstream)
            .await
            .and_then(|name| self.query_nodes.get(&name))
    }

    async fn select_next(
        &self,
        task_notes: &ServerTaskNotes,
        upstream: &UpstreamAddr,
    ) -> ArcEscaper {
        let escaper = self
            .select_query(task_notes, upstream)
            .await
            .unwrap_or(&self.fallback_node);
        Arc::clone(escaper)
    }
}

#[async_trait]
impl Escaper for RouteQueryEscaper {
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    fn ref_route_stats(&self) -> Option<&Arc<RouteEscaperStats>> {
        Some(&self.stats)
    }

    async fn publish(&self, _data: String) -> anyhow::Result<()> {
        Err(anyhow!("not implemented"))
    }

    async fn tcp_setup_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        audit_ctx: &mut AuditContext,
    ) -> TcpConnectResult {
        tcp_notes.escaper.clone_from(&self.config.name);
        let escaper = self.select_next(task_notes, task_conf.upstream).await;
        self.stats.add_request_passed();
        escaper
            .tcp_setup_connection(task_conf, tcp_notes, task_notes, task_stats, audit_ctx)
            .await
    }

    async fn tls_setup_connection(
        &self,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        audit_ctx: &mut AuditContext,
    ) -> TcpConnectResult {
        tcp_notes.escaper.clone_from(&self.config.name);
        let escaper = self.select_next(task_notes, task_conf.tcp.upstream).await;
        self.stats.add_request_passed();
        escaper
            .tls_setup_connection(task_conf, tcp_notes, task_notes, task_stats, audit_ctx)
            .await
    }

    async fn udp_setup_connection(
        &self,
        task_conf: &UdpConnectTaskConf<'_>,
        udp_notes: &mut UdpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult {
        udp_notes.escaper.clone_from(&self.config.name);
        let escaper = self.select_next(task_notes, task_conf.upstream).await;
        self.stats.add_request_passed();
        escaper
            .udp_setup_connection(task_conf, udp_notes, task_notes, task_stats)
            .await
    }

    async fn udp_setup_relay(
        &self,
        task_conf: &UdpRelayTaskConf<'_>,
        udp_notes: &mut UdpRelayTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        udp_notes.escaper.clone_from(&self.config.name);
        let escaper = self.select_next(task_notes, task_conf.initial_peer).await;
        self.stats.add_request_passed();
        escaper
            .udp_setup_relay(task_conf, udp_notes, task_notes, task_stats)
            .await
    }

    fn new_http_forward_context(&self, escaper: ArcEscaper) -> BoxHttpForwardContext {
        let ctx = RouteHttpForwardContext::new(escaper);
        Box::new(ctx)
    }

    async fn new_ftp_connect_context(
        &self,
        _escaper: ArcEscaper,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
    ) -> BoxFtpConnectContext {
        let escaper = self.select_next(task_notes, task_conf.upstream).await;
        self.stats.add_request_passed();
        escaper
            .new_ftp_connect_context(Arc::clone(&escaper), task_conf, task_notes)
            .await
    }
}

#[async_trait]
impl EscaperInternal for RouteQueryEscaper {
    fn _resolver(&self) -> &NodeName {
        Default::default()
    }

    fn _depend_on_escaper(&self, name: &NodeName) -> bool {
        self.query_nodes.contains_key(name) || self.fallback_node.name().eq(name)
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        AnyEscaperConfig::RouteQuery((*self.config).clone())
    }

    fn _reload(
        &self,
        config: AnyEscaperConfig,
        registry: &mut EscaperRegistry,
    ) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::clone(&self.stats);
        RouteQueryEscaper::prepare_reload(config, stats, registry)
    }

    async fn _check_out_next_escaper(
        &self,
        task_notes: &ServerTaskNotes,
        upstream: &UpstreamAddr,
    ) -> Option<ArcEscaper> {
        let escaper = self.select_next(task_notes, upstream).await;
        self.stats.add_request_passed();
        Some(escaper)
    }

    async fn _new_http_forward_connection(
        &self,
        _task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }

    async fn _new_https_forward_connection(
        &self,
        _task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }

    async fn _new_ftp_control_connection(
        &self,
        _task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }

    async fn _new_ftp_transfer_connection(
        &self,
        _task_conf: &TcpConnectTaskConf<'_>,
        transfer_tcp_notes: &mut TcpConnectTaskNotes,
        _control_tcp_notes: &TcpConnectTaskNotes,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteTransferStats,
        _ftp_server: &UpstreamAddr,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        transfer_tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }
}
