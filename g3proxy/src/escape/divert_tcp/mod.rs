/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use slog::Logger;
use tokio::io::AsyncWrite;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_io_ext::LimitedWriteExt;
use g3_resolver::{ResolveError, ResolveLocalError};
use g3_types::collection::{SelectiveVec, SelectiveVecBuilder};
use g3_types::metrics::NodeName;
use g3_types::net::{
    Host, ProxyProtocolEncodeError, ProxyProtocolV2Encoder, UpstreamAddr, WeightedUpstreamAddr,
};

use super::{
    ArcEscaper, ArcEscaperStats, Escaper, EscaperExt, EscaperInternal, EscaperRegistry,
    EscaperStats,
};
use crate::audit::AuditContext;
use crate::auth::UserUpstreamTrafficStats;
use crate::config::escaper::divert_tcp::DivertTcpEscaperConfig;
use crate::config::escaper::{AnyEscaperConfig, EscaperConfig};
use crate::module::ftp_over_http::{
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpConnectContext,
    BoxFtpRemoteConnection, DirectFtpConnectContext,
};
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, BoxHttpForwardContext,
    DirectHttpForwardContext,
};
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectResult, TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectError, UdpConnectResult, UdpConnectTaskConf,
    UdpConnectTaskNotes,
};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupError, UdpRelaySetupResult, UdpRelayTaskConf,
    UdpRelayTaskNotes,
};
use crate::resolve::{ArcIntegratedResolverHandle, HappyEyeballsResolveJob};
use crate::serve::ServerTaskNotes;

mod stats;
use stats::DivertTcpEscaperStats;

mod http_forward;
mod tcp_connect;
mod tls_connect;

pub(super) struct DivertTcpEscaper {
    config: Arc<DivertTcpEscaperConfig>,
    stats: Arc<DivertTcpEscaperStats>,
    proxy_nodes: SelectiveVec<WeightedUpstreamAddr>,
    resolver_handle: Option<ArcIntegratedResolverHandle>,
    escape_logger: Option<Logger>,
}

impl DivertTcpEscaper {
    fn new_obj(
        config: DivertTcpEscaperConfig,
        stats: Arc<DivertTcpEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        let mut nodes_builder = SelectiveVecBuilder::new();
        for node in &config.proxy_nodes {
            nodes_builder.insert(node.clone());
        }
        let proxy_nodes = nodes_builder
            .build()
            .ok_or_else(|| anyhow!("no next proxy node set"))?;

        let escape_logger = config.get_escape_logger();

        let resolver = config.resolver();
        let resolver_handle = if resolver.is_empty() {
            None
        } else {
            Some(crate::resolve::get_handle(resolver)?)
        };

        stats.set_extra_tags(config.extra_metrics_tags.clone());

        let escaper = DivertTcpEscaper {
            config: Arc::new(config),
            stats,
            proxy_nodes,
            resolver_handle,
            escape_logger,
        };

        Ok(Arc::new(escaper))
    }

    pub(super) fn prepare_initial(config: DivertTcpEscaperConfig) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::new(DivertTcpEscaperStats::new(config.name()));
        DivertTcpEscaper::new_obj(config, stats)
    }

    fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<DivertTcpEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::DivertTcp(config) = config {
            DivertTcpEscaper::new_obj(config, stats)
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn get_next_proxy(&self, task_notes: &ServerTaskNotes, target_host: &Host) -> &UpstreamAddr {
        self.select_consistent(
            &self.proxy_nodes,
            self.config.proxy_pick_policy,
            task_notes,
            target_host,
        )
        .inner()
    }

    fn resolve_happy(&self, domain: Arc<str>) -> Result<HappyEyeballsResolveJob, ResolveError> {
        if let Some(resolver_handle) = &self.resolver_handle {
            HappyEyeballsResolveJob::new_dyn(self.config.resolve_strategy, resolver_handle, domain)
        } else {
            Err(ResolveLocalError::NoResolverSet.into())
        }
    }

    fn fetch_user_upstream_io_stats(
        &self,
        task_notes: &ServerTaskNotes,
    ) -> Vec<Arc<UserUpstreamTrafficStats>> {
        task_notes
            .user_ctx()
            .map(|ctx| ctx.fetch_upstream_traffic_stats(self.name(), self.stats.share_extra_tags()))
            .unwrap_or_default()
    }

    fn encode_pp2_tlv(
        &self,
        pp2_encoder: &mut ProxyProtocolV2Encoder,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        tls_name: Option<&Host>,
    ) -> Result<(), ProxyProtocolEncodeError> {
        pp2_encoder.push_upstream(task_conf.upstream)?;
        if let Some(tls_name) = tls_name {
            pp2_encoder.push_tls_name(tls_name)?;
        }
        if let Some(user_ctx) = task_notes.user_ctx() {
            pp2_encoder.push_username(user_ctx.user_name())?;
        }
        pp2_encoder.push_task_id(task_notes.id.as_bytes())?;
        Ok(())
    }

    async fn send_pp2_header<W>(
        &self,
        writer: &mut W,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        tls_name: Option<&Host>,
    ) -> Result<usize, TcpConnectError>
    where
        W: AsyncWrite + Unpin,
    {
        let mut pp2_encoder =
            ProxyProtocolV2Encoder::new_tcp(task_notes.client_addr(), task_notes.server_addr())?;
        self.encode_pp2_tlv(&mut pp2_encoder, task_conf, task_notes, tls_name)?;

        let pp2_data = pp2_encoder.finalize();
        writer
            .write_all_flush(pp2_data)
            .await
            .map_err(TcpConnectError::ProxyProtocolWriteFailed)?;
        Ok(pp2_data.len())
    }
}

impl EscaperExt for DivertTcpEscaper {}

#[async_trait]
impl Escaper for DivertTcpEscaper {
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    fn get_escape_stats(&self) -> Option<ArcEscaperStats> {
        Some(self.stats.clone())
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
        _audit_ctx: &mut AuditContext,
    ) -> TcpConnectResult {
        self.stats.interface.add_tcp_connect_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.tcp_new_connection(task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn tls_setup_connection(
        &self,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        _audit_ctx: &mut AuditContext,
    ) -> TcpConnectResult {
        self.stats.interface.add_tls_connect_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.tls_new_connection(task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn udp_setup_connection(
        &self,
        _task_conf: &UdpConnectTaskConf<'_>,
        udp_notes: &mut UdpConnectTaskNotes,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult {
        self.stats.interface.add_udp_connect_attempted();
        udp_notes.escaper.clone_from(&self.config.name);
        Err(UdpConnectError::MethodUnavailable)
    }

    async fn udp_setup_relay(
        &self,
        _task_conf: &UdpRelayTaskConf<'_>,
        udp_notes: &mut UdpRelayTaskNotes,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        self.stats.interface.add_udp_relay_session_attempted();
        udp_notes.escaper.clone_from(&self.config.name);
        Err(UdpRelaySetupError::MethodUnavailable)
    }

    fn new_http_forward_context(&self, escaper: ArcEscaper) -> BoxHttpForwardContext {
        let ctx = DirectHttpForwardContext::new(self.stats.clone(), escaper);
        Box::new(ctx)
    }

    async fn new_ftp_connect_context(
        &self,
        escaper: ArcEscaper,
        task_conf: &TcpConnectTaskConf<'_>,
        _task_notes: &ServerTaskNotes,
    ) -> BoxFtpConnectContext {
        Box::new(DirectFtpConnectContext::new(
            escaper,
            task_conf.upstream.clone(),
        ))
    }
}

#[async_trait]
impl EscaperInternal for DivertTcpEscaper {
    fn _resolver(&self) -> &NodeName {
        self.config.resolver()
    }

    fn _depend_on_escaper(&self, _name: &NodeName) -> bool {
        false
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        AnyEscaperConfig::DivertTcp(self.config.as_ref().clone())
    }

    fn _reload(
        &self,
        config: AnyEscaperConfig,
        _registry: &mut EscaperRegistry,
    ) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::clone(&self.stats);
        DivertTcpEscaper::prepare_reload(config, stats)
    }

    async fn _new_http_forward_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.stats.interface.add_http_forward_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.http_forward_new_connection(task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn _new_https_forward_connection(
        &self,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.stats
            .interface
            .add_https_forward_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.https_forward_new_connection(task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn _new_ftp_control_connection(
        &self,
        _task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        self.stats.interface.add_ftp_over_http_request_attempted();
        self.stats.interface.add_ftp_control_connection_attempted();
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
        self.stats.interface.add_ftp_transfer_connection_attempted();
        transfer_tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }
}
