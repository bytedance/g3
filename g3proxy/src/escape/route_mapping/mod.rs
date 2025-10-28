/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::metrics::NodeName;
use g3_types::net::UpstreamAddr;

use super::{ArcEscaper, Escaper, EscaperInternal, EscaperRegistry, RouteEscaperStats};
use crate::audit::AuditContext;
use crate::config::escaper::route_mapping::RouteMappingEscaperConfig;
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

pub(super) struct RouteMappingEscaper {
    config: RouteMappingEscaperConfig,
    stats: Arc<RouteEscaperStats>,
    next_nodes: Vec<ArcEscaper>,
}

impl RouteMappingEscaper {
    fn new_obj<F>(
        config: RouteMappingEscaperConfig,
        stats: Arc<RouteEscaperStats>,
        mut fetch_escaper: F,
    ) -> ArcEscaper
    where
        F: FnMut(&NodeName) -> ArcEscaper,
    {
        let mut next_nodes = Vec::with_capacity(config.next_nodes.len());
        for name in &config.next_nodes {
            let escaper = fetch_escaper(name);
            next_nodes.push(escaper)
        }

        let escaper = RouteMappingEscaper {
            config,
            stats,
            next_nodes,
        };

        Arc::new(escaper)
    }

    pub(super) fn prepare_initial(config: RouteMappingEscaperConfig) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::new(RouteEscaperStats::new(config.name()));
        Ok(RouteMappingEscaper::new_obj(
            config,
            stats,
            super::registry::get_or_insert_default,
        ))
    }

    fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<RouteEscaperStats>,
        registry: &mut EscaperRegistry,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::RouteMapping(config) = config {
            Ok(RouteMappingEscaper::new_obj(config, stats, |name| {
                registry.get_or_insert_default(name)
            }))
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn random_next(&self) -> ArcEscaper {
        fastrand::choice(&self.next_nodes)
            .unwrap_or(&self.next_nodes[0])
            .clone()
    }

    fn select_next(&self, task_notes: &ServerTaskNotes) -> ArcEscaper {
        if let Some(i) = task_notes.egress_path_number_id(self.name(), self.next_nodes.len()) {
            return self.next_nodes[i].clone();
        }
        self.random_next()
    }
}

#[async_trait]
impl Escaper for RouteMappingEscaper {
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    fn ref_route_stats(&self) -> Option<&Arc<RouteEscaperStats>> {
        Some(&self.stats)
    }

    async fn publish(&self, _data: &str) -> anyhow::Result<()> {
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
        let escaper = self.select_next(task_notes);
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
        let escaper = self.select_next(task_notes);
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
        let escaper = self.select_next(task_notes);
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
        let escaper = self.select_next(task_notes);
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
        let escaper = self.select_next(task_notes);
        self.stats.add_request_passed();
        escaper
            .new_ftp_connect_context(Arc::clone(&escaper), task_conf, task_notes)
            .await
    }
}

#[async_trait]
impl EscaperInternal for RouteMappingEscaper {
    fn _resolver(&self) -> &NodeName {
        Default::default()
    }

    fn _depend_on_escaper(&self, name: &NodeName) -> bool {
        self.config.next_nodes.contains(name)
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        AnyEscaperConfig::RouteMapping(self.config.clone())
    }

    fn _reload(
        &self,
        config: AnyEscaperConfig,
        registry: &mut EscaperRegistry,
    ) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::clone(&self.stats);
        RouteMappingEscaper::prepare_reload(config, stats, registry)
    }

    async fn _check_out_next_escaper(
        &self,
        task_notes: &ServerTaskNotes,
        _upstream: &UpstreamAddr,
    ) -> Option<ArcEscaper> {
        let escaper = self.select_next(task_notes);
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
