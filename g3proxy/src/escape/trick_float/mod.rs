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

use std::collections::BTreeSet;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use rand::seq::SliceRandom;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::metrics::MetricsName;
use g3_types::net::{Host, OpensslClientConfig, UpstreamAddr};

use super::{ArcEscaper, Escaper, EscaperInternal, RouteEscaperStats};
use crate::audit::AuditContext;
use crate::config::escaper::trick_float::TrickFloatEscaperConfig;
use crate::config::escaper::{AnyEscaperConfig, EscaperConfig};
use crate::module::ftp_over_http::{
    AnyFtpConnectContextParam, ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats,
    BoxFtpConnectContext, BoxFtpRemoteConnection, DenyFtpConnectContext,
};
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, BoxHttpForwardContext,
    RouteHttpForwardContext,
};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectResult, TcpConnectTaskNotes};
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectError, UdpConnectResult, UdpConnectTaskNotes,
};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupError, UdpRelaySetupResult, UdpRelayTaskNotes,
};
use crate::serve::ServerTaskNotes;

pub(super) struct TrickFloatEscaper {
    config: TrickFloatEscaperConfig,
    stats: Arc<RouteEscaperStats>,
    next_nodes: Vec<ArcEscaper>,
}

impl TrickFloatEscaper {
    fn new_obj(config: TrickFloatEscaperConfig, stats: Arc<RouteEscaperStats>) -> ArcEscaper {
        let mut next_nodes = Vec::with_capacity(config.next_nodes.len());
        for name in &config.next_nodes {
            let escaper = super::registry::get_or_insert_default(name);
            next_nodes.push(escaper);
        }

        let escaper = TrickFloatEscaper {
            config,
            stats,
            next_nodes,
        };

        Arc::new(escaper)
    }

    pub(super) fn prepare_initial(config: TrickFloatEscaperConfig) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::new(RouteEscaperStats::new(config.name()));
        Ok(TrickFloatEscaper::new_obj(config, stats))
    }

    fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<RouteEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::TrickFloat(config) = config {
            Ok(TrickFloatEscaper::new_obj(config, stats))
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn random_next(&self) -> anyhow::Result<ArcEscaper> {
        let mut rng = rand::thread_rng();
        let escaper = self
            .next_nodes
            .choose_weighted(&mut rng, |escaper| escaper._trick_float_weight())
            .map_err(|e| anyhow!("no next escaper can be selected: {e}"))?;
        Ok(Arc::clone(escaper))
    }
}

#[async_trait]
impl Escaper for TrickFloatEscaper {
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
        match self.random_next() {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .tcp_setup_connection(tcp_notes, task_notes, task_stats, audit_ctx)
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Err(TcpConnectError::EscaperNotUsable(e))
            }
        }
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
        match self.random_next() {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .tls_setup_connection(
                        tcp_notes, task_notes, task_stats, audit_ctx, tls_config, tls_name,
                    )
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Err(TcpConnectError::EscaperNotUsable(e))
            }
        }
    }

    async fn udp_setup_connection<'a>(
        &'a self,
        udp_notes: &'a mut UdpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult {
        udp_notes.escaper.clone_from(&self.config.name);
        match self.random_next() {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .udp_setup_connection(udp_notes, task_notes, task_stats)
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Err(UdpConnectError::EscaperNotUsable(e))
            }
        }
    }

    async fn udp_setup_relay<'a>(
        &'a self,
        udp_notes: &'a mut UdpRelayTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        udp_notes.escaper.clone_from(&self.config.name);
        match self.random_next() {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .udp_setup_relay(udp_notes, task_notes, task_stats)
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Err(UdpRelaySetupError::EscaperNotUsable(e))
            }
        }
    }

    fn new_http_forward_context(&self, escaper: ArcEscaper) -> BoxHttpForwardContext {
        let ctx = RouteHttpForwardContext::new(escaper);
        Box::new(ctx)
    }

    async fn new_ftp_connect_context<'a>(
        &'a self,
        _escaper: ArcEscaper,
        task_notes: &'a ServerTaskNotes,
        upstream: &'a UpstreamAddr,
    ) -> BoxFtpConnectContext {
        match self.random_next() {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .new_ftp_connect_context(Arc::clone(&escaper), task_notes, upstream)
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Box::new(DenyFtpConnectContext::new(
                    self.name(),
                    Some(TcpConnectError::EscaperNotUsable(e)),
                ))
            }
        }
    }
}

#[async_trait]
impl EscaperInternal for TrickFloatEscaper {
    fn _resolver(&self) -> &MetricsName {
        Default::default()
    }

    fn _dependent_escaper(&self) -> Option<BTreeSet<MetricsName>> {
        self.config.dependent_escaper()
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        AnyEscaperConfig::TrickFloat(self.config.clone())
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
        TrickFloatEscaper::prepare_reload(config, stats)
    }

    async fn _check_out_next_escaper(
        &self,
        _task_notes: &ServerTaskNotes,
        _upstream: &UpstreamAddr,
    ) -> Option<ArcEscaper> {
        if let Ok(escaper) = self.random_next() {
            self.stats.add_request_passed();
            Some(escaper)
        } else {
            self.stats.add_request_failed();
            None
        }
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
