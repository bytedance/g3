/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::collections::BTreeSet;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use async_trait::async_trait;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::metrics::MetricsName;
use g3_types::net::{Host, OpensslClientConfig, UpstreamAddr};

use super::{ArcEscaper, Escaper, EscaperInternal, RouteEscaperStats};
use crate::audit::{AuditContext, AuditHandle};
use crate::config::escaper::comply_audit::ComplyAuditEscaperConfig;
use crate::config::escaper::{AnyEscaperConfig, EscaperConfig};
use crate::module::ftp_over_http::{
    AnyFtpConnectContextParam, ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats,
    BoxFtpConnectContext, BoxFtpRemoteConnection,
};
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, BoxHttpForwardContext,
    RouteHttpForwardContext,
};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectResult, TcpConnectTaskNotes};
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectResult, UdpConnectTaskNotes,
};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupResult, UdpRelayTaskNotes,
};
use crate::serve::ServerTaskNotes;

pub(super) struct ComplyAuditEscaper {
    config: ComplyAuditEscaperConfig,
    stats: Arc<RouteEscaperStats>,
    next: ArcEscaper,
    audit_handle: Arc<AuditHandle>,
}

impl ComplyAuditEscaper {
    fn new_obj(
        config: ComplyAuditEscaperConfig,
        stats: Arc<RouteEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        let next = super::registry::get_or_insert_default(&config.next);
        let auditor = crate::audit::get_or_insert_default(&config.auditor);
        let audit_handle = auditor
            .build_handle()
            .context("failed to build audit handle")?;

        let escaper = ComplyAuditEscaper {
            config,
            stats,
            next,
            audit_handle,
        };
        Ok(Arc::new(escaper))
    }

    pub(super) fn prepare_initial(config: ComplyAuditEscaperConfig) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::new(RouteEscaperStats::new(config.name()));
        ComplyAuditEscaper::new_obj(config, stats)
    }

    fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<RouteEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::ComplyAudit(config) = config {
            ComplyAuditEscaper::new_obj(config, stats)
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }
}

#[async_trait]
impl Escaper for ComplyAuditEscaper {
    fn name(&self) -> &MetricsName {
        self.config.name()
    }

    fn escaper_type(&self) -> &str {
        self.config.escaper_type()
    }

    fn ref_route_stats(&self) -> Option<&Arc<RouteEscaperStats>> {
        Some(&self.stats)
    }

    async fn publish(&self, _data: String) -> anyhow::Result<()> {
        Err(anyhow!("not implemented"))
    }

    async fn tcp_setup_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        audit_ctx: &'a mut AuditContext,
    ) -> TcpConnectResult {
        tcp_notes.escaper.clone_from(&self.config.name);
        self.stats.add_request_passed();
        self._update_audit_context(audit_ctx);
        self.next
            .tcp_setup_connection(tcp_notes, task_notes, task_stats, audit_ctx)
            .await
    }

    async fn tls_setup_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        audit_ctx: &'a mut AuditContext,
        tls_config: &'a OpensslClientConfig,
        tls_name: &'a Host,
    ) -> TcpConnectResult {
        tcp_notes.escaper.clone_from(&self.config.name);
        self.stats.add_request_passed();
        self._update_audit_context(audit_ctx);
        self.next
            .tls_setup_connection(
                tcp_notes, task_notes, task_stats, audit_ctx, tls_config, tls_name,
            )
            .await
    }

    async fn udp_setup_connection<'a>(
        &'a self,
        udp_notes: &'a mut UdpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult {
        udp_notes.escaper.clone_from(&self.config.name);
        self.stats.add_request_passed();
        self.next
            .udp_setup_connection(udp_notes, task_notes, task_stats)
            .await
    }

    async fn udp_setup_relay<'a>(
        &'a self,
        udp_notes: &'a mut UdpRelayTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        udp_notes.escaper.clone_from(&self.config.name);
        self.stats.add_request_passed();
        self.next
            .udp_setup_relay(udp_notes, task_notes, task_stats)
            .await
    }

    fn new_http_forward_context(&self, escaper: ArcEscaper) -> BoxHttpForwardContext {
        let ctx = RouteHttpForwardContext::new(escaper);
        Box::new(ctx)
    }

    async fn new_ftp_connect_context<'a>(
        &'a self,
        escaper: ArcEscaper,
        task_notes: &'a ServerTaskNotes,
        upstream: &'a UpstreamAddr,
    ) -> BoxFtpConnectContext {
        self.stats.add_request_passed();
        self.next
            .new_ftp_connect_context(Arc::clone(&escaper), task_notes, upstream)
            .await
    }
}

#[async_trait]
impl EscaperInternal for ComplyAuditEscaper {
    fn _resolver(&self) -> &MetricsName {
        Default::default()
    }

    fn _auditor(&self) -> Option<&MetricsName> {
        Some(&self.config.auditor)
    }

    fn _dependent_escaper(&self) -> Option<BTreeSet<MetricsName>> {
        let mut set = BTreeSet::new();
        set.insert(self.config.next.clone());
        Some(set)
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        AnyEscaperConfig::ComplyAudit(self.config.clone())
    }

    fn _update_config_in_place(
        &self,
        _flags: u64,
        _config: AnyEscaperConfig,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn _lock_safe_reload(&self, config: AnyEscaperConfig) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::clone(&self.stats);
        ComplyAuditEscaper::prepare_reload(config, stats)
    }

    async fn _check_out_next_escaper(
        &self,
        _task_notes: &ServerTaskNotes,
        _upstream: &UpstreamAddr,
    ) -> Option<ArcEscaper> {
        self.stats.add_request_passed();
        Some(self.next.clone())
    }

    fn _update_audit_context(&self, audit_ctx: &mut AuditContext) {
        audit_ctx.set_handle(self.audit_handle.clone());
    }

    async fn _new_http_forward_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        _task_notes: &'a ServerTaskNotes,
        _task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }

    async fn _new_https_forward_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        _task_notes: &'a ServerTaskNotes,
        _task_stats: ArcHttpForwardTaskRemoteStats,
        _tls_config: &'a OpensslClientConfig,
        _tls_name: &'a Host,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }

    async fn _new_ftp_control_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        _task_notes: &'a ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }

    async fn _new_ftp_transfer_connection<'a>(
        &'a self,
        transfer_tcp_notes: &'a mut TcpConnectTaskNotes,
        _control_tcp_notes: &'a TcpConnectTaskNotes,
        _task_notes: &'a ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteTransferStats,
        _context: AnyFtpConnectContextParam,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        transfer_tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }
}
