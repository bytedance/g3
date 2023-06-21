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
use slog::Logger;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_resolver::{ResolveError, ResolveLocalError};
use g3_types::collection::{SelectiveVec, SelectiveVecBuilder};
use g3_types::metrics::MetricsName;
use g3_types::net::{
    Host, HttpForwardCapability, OpensslTlsClientConfig, UpstreamAddr, WeightedUpstreamAddr,
};

use super::{
    ArcEscaper, ArcEscaperInternalStats, ArcEscaperStats, Escaper, EscaperExt, EscaperInternal,
    EscaperStats,
};
use crate::auth::UserUpstreamTrafficStats;
use crate::config::escaper::proxy_http::ProxyHttpEscaperConfig;
use crate::config::escaper::{AnyEscaperConfig, EscaperConfig};
use crate::module::ftp_over_http::{
    AnyFtpConnectContextParam, ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats,
    BoxFtpConnectContext, BoxFtpRemoteConnection, DirectFtpConnectContext,
    DirectFtpConnectContextParam,
};
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, BoxHttpForwardContext,
    ProxyHttpForwardContext,
};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectResult, TcpConnectTaskNotes};
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectError, UdpConnectResult, UdpConnectTaskNotes,
};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupError, UdpRelaySetupResult, UdpRelayTaskNotes,
};
use crate::resolve::{ArcIntegratedResolverHandle, HappyEyeballsResolveJob};
use crate::serve::ServerTaskNotes;

mod stats;
use stats::ProxyHttpEscaperStats;

mod http_connect;
mod http_forward;
mod tcp_connect;

pub(super) struct ProxyHttpEscaper {
    config: Arc<ProxyHttpEscaperConfig>,
    stats: Arc<ProxyHttpEscaperStats>,
    proxy_nodes: SelectiveVec<WeightedUpstreamAddr>,
    resolver_handle: Option<ArcIntegratedResolverHandle>,
    escape_logger: Logger,
}

impl ProxyHttpEscaper {
    fn new_obj(
        config: ProxyHttpEscaperConfig,
        stats: Arc<ProxyHttpEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        let mut nodes_builder = SelectiveVecBuilder::new();
        for node in &config.proxy_nodes {
            nodes_builder.insert(node.clone());
        }
        let proxy_nodes = nodes_builder
            .build()
            .map_err(|e| anyhow!("failed to build proxy_addr selector: {e:?}"))?;

        let escape_logger = config.get_escape_logger();

        let resolver = config.resolver();
        let resolver_handle = if resolver.is_empty() {
            None
        } else {
            Some(crate::resolve::get_handle(resolver)?)
        };

        stats.set_extra_tags(config.extra_metrics_tags.clone());

        let escaper = ProxyHttpEscaper {
            config: Arc::new(config),
            stats,
            proxy_nodes,
            resolver_handle,
            escape_logger,
        };

        Ok(Arc::new(escaper))
    }

    pub(super) fn prepare_initial(config: AnyEscaperConfig) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::ProxyHttp(config) = config {
            let stats = Arc::new(ProxyHttpEscaperStats::new(config.name()));
            ProxyHttpEscaper::new_obj(*config, stats)
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<ProxyHttpEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::ProxyHttp(config) = config {
            ProxyHttpEscaper::new_obj(*config, stats)
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn get_next_proxy<'a>(
        &'a self,
        task_notes: &'a ServerTaskNotes,
        target_host: &'a Host,
    ) -> &'a UpstreamAddr {
        self.select_consistent(
            &self.proxy_nodes,
            self.config.proxy_pick_policy,
            task_notes,
            target_host,
        )
        .inner()
    }

    fn resolve_happy(&self, domain: &str) -> Result<HappyEyeballsResolveJob, ResolveError> {
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
            .map(|ctx| ctx.fetch_upstream_traffic_stats(self.name(), self.stats.extra_tags()))
            .unwrap_or_default()
    }
}

impl EscaperExt for ProxyHttpEscaper {}

#[async_trait]
impl Escaper for ProxyHttpEscaper {
    fn name(&self) -> &MetricsName {
        self.config.name()
    }

    fn escaper_type(&self) -> &str {
        self.config.escaper_type()
    }

    fn get_escape_stats(&self) -> Option<ArcEscaperStats> {
        Some(Arc::clone(&self.stats) as ArcEscaperStats)
    }

    async fn publish(&self, _data: String) -> anyhow::Result<()> {
        Err(anyhow!("not implemented"))
    }

    async fn tcp_setup_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        self.stats.interface.add_tcp_connect_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.http_connect_new_tcp_connection(tcp_notes, task_notes, task_stats)
            .await
    }

    async fn tls_setup_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        tls_config: &'a OpensslTlsClientConfig,
        tls_name: &'a str,
    ) -> TcpConnectResult {
        self.stats.interface.add_tls_connect_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.http_connect_new_tls_connection(
            tcp_notes, task_notes, task_stats, tls_config, tls_name,
        )
        .await
    }

    async fn udp_setup_connection<'a>(
        &'a self,
        udp_notes: &'a mut UdpConnectTaskNotes,
        _task_notes: &'a ServerTaskNotes,
        _task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult {
        self.stats.interface.add_udp_connect_attempted();
        udp_notes.escaper.clone_from(&self.config.name);
        Err(UdpConnectError::MethodUnavailable)
    }

    async fn udp_setup_relay<'a>(
        &'a self,
        udp_notes: &'a mut UdpRelayTaskNotes,
        _task_notes: &'a ServerTaskNotes,
        _task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        self.stats.interface.add_udp_relay_session_attempted();
        udp_notes.escaper.clone_from(&self.config.name);
        Err(UdpRelaySetupError::MethodUnavailable)
    }

    fn new_http_forward_context(&self, escaper: ArcEscaper) -> BoxHttpForwardContext {
        let ctx = ProxyHttpForwardContext::new(
            Arc::clone(&self.stats) as ArcEscaperInternalStats,
            escaper,
        );
        Box::new(ctx)
    }

    async fn new_ftp_connect_context<'a>(
        &'a self,
        escaper: ArcEscaper,
        _task_notes: &'a ServerTaskNotes,
        upstream: &'a UpstreamAddr,
    ) -> BoxFtpConnectContext {
        Box::new(DirectFtpConnectContext::new(escaper, upstream.clone()))
    }
}

#[async_trait]
impl EscaperInternal for ProxyHttpEscaper {
    fn _resolver(&self) -> &MetricsName {
        self.config.resolver()
    }

    fn _dependent_escaper(&self) -> Option<BTreeSet<MetricsName>> {
        None
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        let config = &*self.config;
        AnyEscaperConfig::ProxyHttp(Box::new(config.clone()))
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
        ProxyHttpEscaper::prepare_reload(config, stats)
    }

    #[inline]
    fn _local_http_forward_capability(&self) -> HttpForwardCapability {
        self.config.http_forward_capability
    }

    async fn _check_out_next_escaper(
        &self,
        _task_notes: &ServerTaskNotes,
        _upstream: &UpstreamAddr,
    ) -> Option<ArcEscaper> {
        None
    }

    async fn _new_http_forward_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.stats.interface.add_http_forward_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.http_forward_new_connection(tcp_notes, task_notes, task_stats)
            .await
    }

    async fn _new_https_forward_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
        tls_config: &'a OpensslTlsClientConfig,
        tls_name: &'a str,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.stats
            .interface
            .add_https_forward_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.https_forward_new_connection(tcp_notes, task_notes, task_stats, tls_config, tls_name)
            .await
    }

    async fn _new_ftp_control_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        _task_notes: &'a ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        self.stats.interface.add_ftp_over_http_request_attempted();
        self.stats.interface.add_ftp_control_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }

    async fn _new_ftp_transfer_connection<'a>(
        &'a self,
        transfer_tcp_notes: &'a mut TcpConnectTaskNotes,
        _control_tcp_notes: &'a TcpConnectTaskNotes,
        _task_notes: &'a ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteTransferStats,
        mut context: AnyFtpConnectContextParam,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        self.stats.interface.add_ftp_transfer_connection_attempted();
        transfer_tcp_notes.escaper.clone_from(&self.config.name);
        match context.downcast_mut::<DirectFtpConnectContextParam>() {
            Some(_ctx) => Err(TcpConnectError::MethodUnavailable),
            None => Err(TcpConnectError::EscaperNotUsable),
        }
    }
}
