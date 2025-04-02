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

use anyhow::{Context, anyhow};
use async_trait::async_trait;
use slog::Logger;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_resolver::{ResolveError, ResolveLocalError};
use g3_types::collection::{SelectiveVec, SelectiveVecBuilder};
use g3_types::metrics::NodeName;
use g3_types::net::{
    Host, HttpForwardCapability, OpensslClientConfig, UpstreamAddr, WeightedUpstreamAddr,
};

use super::{ArcEscaper, ArcEscaperStats, Escaper, EscaperExt, EscaperInternal, EscaperStats};
use crate::audit::AuditContext;
use crate::auth::UserUpstreamTrafficStats;
use crate::config::escaper::proxy_https::ProxyHttpsEscaperConfig;
use crate::config::escaper::{AnyEscaperConfig, EscaperConfig};
use crate::module::ftp_over_http::{
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpConnectContext,
    BoxFtpRemoteConnection, DirectFtpConnectContext,
};
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, BoxHttpForwardContext,
    ProxyHttpForwardContext,
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
use stats::ProxyHttpsEscaperStats;

mod http_connect;
mod http_forward;
mod tcp_connect;
mod tls_handshake;

pub(super) struct ProxyHttpsEscaper {
    config: Arc<ProxyHttpsEscaperConfig>,
    stats: Arc<ProxyHttpsEscaperStats>,
    proxy_nodes: SelectiveVec<WeightedUpstreamAddr>,
    tls_config: OpensslClientConfig,
    resolver_handle: Option<ArcIntegratedResolverHandle>,
    escape_logger: Logger,
}

impl ProxyHttpsEscaper {
    fn new_obj(
        config: ProxyHttpsEscaperConfig,
        stats: Arc<ProxyHttpsEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        let mut nodes_builder = SelectiveVecBuilder::new();
        for node in &config.proxy_nodes {
            nodes_builder.insert(node.clone());
        }
        let proxy_nodes = nodes_builder
            .build()
            .ok_or_else(|| anyhow!("no next proxy node set"))?;

        let tls_config = config
            .tls_config
            .build()
            .context("failed to build tls config")?;

        let escape_logger = config.get_escape_logger();

        let resolver = config.resolver();
        let resolver_handle = if resolver.is_empty() {
            None
        } else {
            Some(crate::resolve::get_handle(resolver)?)
        };

        stats.set_extra_tags(config.extra_metrics_tags.clone());

        let escaper = ProxyHttpsEscaper {
            config: Arc::new(config),
            stats,
            proxy_nodes,
            tls_config,
            resolver_handle,
            escape_logger,
        };
        Ok(Arc::new(escaper))
    }

    pub(super) fn prepare_initial(config: ProxyHttpsEscaperConfig) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::new(ProxyHttpsEscaperStats::new(config.name()));
        ProxyHttpsEscaper::new_obj(config, stats)
    }

    fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<ProxyHttpsEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::ProxyHttps(config) = config {
            ProxyHttpsEscaper::new_obj(config, stats)
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
}

impl EscaperExt for ProxyHttpsEscaper {}

#[async_trait]
impl Escaper for ProxyHttpsEscaper {
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    fn escaper_type(&self) -> &str {
        self.config.escaper_type()
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
        self.http_connect_new_tcp_connection(task_conf, tcp_notes, task_notes, task_stats)
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
        self.http_connect_new_tls_connection(task_conf, tcp_notes, task_notes, task_stats)
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
        let ctx = ProxyHttpForwardContext::new(self.stats.clone(), escaper);
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
impl EscaperInternal for ProxyHttpsEscaper {
    fn _resolver(&self) -> &NodeName {
        self.config.resolver()
    }

    fn _dependent_escaper(&self) -> Option<BTreeSet<NodeName>> {
        None
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        let config = &*self.config;
        AnyEscaperConfig::ProxyHttps(config.clone())
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
        ProxyHttpsEscaper::prepare_reload(config, stats)
    }

    #[inline]
    fn _local_http_forward_capability(&self) -> HttpForwardCapability {
        self.config.http_forward_capability
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
