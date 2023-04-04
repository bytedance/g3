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

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use ip_network_table::IpNetworkTable;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_resolver::ResolveError;
use g3_types::metrics::MetricsName;
use g3_types::net::{Host, OpensslTlsClientConfig, UpstreamAddr};

use super::{ArcEscaper, Escaper, EscaperInternal, RouteEscaperStats};
use crate::config::escaper::route_resolved::RouteResolvedEscaperConfig;
use crate::config::escaper::{AnyEscaperConfig, EscaperConfig};
use crate::module::ftp_over_http::{
    AnyFtpConnectContextParam, ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats,
    BoxFtpConnectContext, BoxFtpRemoteHttpConnection, DenyFtpConnectContext,
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
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupResult, UdpRelayTaskNotes,
};
use crate::resolve::{ArcIntegratedResolverHandle, HappyEyeballsResolveJob};
use crate::serve::ServerTaskNotes;

pub(super) struct RouteResolvedEscaper {
    config: RouteResolvedEscaperConfig,
    stats: Arc<RouteEscaperStats>,
    resolver_handle: ArcIntegratedResolverHandle,
    next_table: BTreeMap<MetricsName, ArcEscaper>,
    lpm_table: IpNetworkTable<ArcEscaper>,
    default_next: ArcEscaper,
}

impl RouteResolvedEscaper {
    fn new_obj(
        config: RouteResolvedEscaperConfig,
        stats: Arc<RouteEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        let resolver_handle = crate::resolve::get_handle(config.resolver())?;

        let mut next_table = BTreeMap::new();
        if let Some(escapers) = config.dependent_escaper() {
            for escaper in escapers {
                let next = super::registry::get_or_insert_default(&escaper);
                next_table.insert(escaper, next);
            }
        }

        let default_next = Arc::clone(next_table.get(&config.default_next).unwrap());

        let mut lpm_table = IpNetworkTable::new();
        for (escaper, networks) in &config.lpm_rules {
            let next = next_table.get(escaper).unwrap();
            for net in networks {
                lpm_table.insert(*net, Arc::clone(next));
            }
        }

        let escaper = RouteResolvedEscaper {
            config,
            stats,
            resolver_handle,
            next_table,
            lpm_table,
            default_next,
        };

        Ok(Arc::new(escaper))
    }

    pub(super) fn prepare_initial(config: AnyEscaperConfig) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::RouteResolved(config) = config {
            let stats = Arc::new(RouteEscaperStats::new(config.name()));
            RouteResolvedEscaper::new_obj(config, stats)
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<RouteEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::RouteResolved(config) = config {
            RouteResolvedEscaper::new_obj(config, stats)
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    async fn get_upstream_ip(&self, ups: &Host) -> Result<IpAddr, ResolveError> {
        match ups {
            Host::Ip(ip) => Ok(*ip),
            Host::Domain(domain) => {
                let mut resolver_job = HappyEyeballsResolveJob::new_dyn(
                    self.config.resolve_strategy,
                    &self.resolver_handle,
                    domain,
                )?;
                let v = resolver_job
                    .get_r1_or_first(self.config.resolution_delay, usize::MAX)
                    .await?;
                self.config.resolve_strategy.pick_best(v).ok_or_else(|| {
                    ResolveError::UnexpectedError(
                        "resolver job return ok but with no ip can be selected".to_string(),
                    )
                })
            }
        }
    }

    fn select_next_by_ip(&self, ip: IpAddr) -> ArcEscaper {
        if !self.lpm_table.is_empty() {
            if let Some((_net, escaper)) = self.lpm_table.longest_match(ip) {
                return Arc::clone(escaper);
            }
        }

        Arc::clone(&self.default_next)
    }

    async fn select_next(&self, ups: &UpstreamAddr) -> Result<ArcEscaper, ResolveError> {
        let ip = self.get_upstream_ip(ups.host()).await?;

        let escaper = self.select_next_by_ip(ip);
        Ok(escaper)
    }
}

#[async_trait]
impl Escaper for RouteResolvedEscaper {
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
    ) -> TcpConnectResult {
        tcp_notes.escaper.clone_from(&self.config.name);
        match self.select_next(&tcp_notes.upstream).await {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .tcp_setup_connection(tcp_notes, task_notes, task_stats)
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Err(e.into())
            }
        }
    }

    async fn tls_setup_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        tls_config: &'a OpensslTlsClientConfig,
        tls_name: &'a str,
    ) -> TcpConnectResult {
        tcp_notes.escaper.clone_from(&self.config.name);
        match self.select_next(&tcp_notes.upstream).await {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .tls_setup_connection(tcp_notes, task_notes, task_stats, tls_config, tls_name)
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Err(e.into())
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
        let upstream = udp_notes
            .upstream
            .as_ref()
            .ok_or(UdpConnectError::NoUpstreamSupplied)?;
        match self.select_next(upstream).await {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .udp_setup_connection(udp_notes, task_notes, task_stats)
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Err(e.into())
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
        match self.select_next(&udp_notes.initial_peer).await {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .udp_setup_relay(udp_notes, task_notes, task_stats)
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Err(e.into())
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
        match self.select_next(upstream).await {
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
                    Some(TcpConnectError::ResolveFailed(e)),
                ))
            }
        }
    }
}

#[async_trait]
impl EscaperInternal for RouteResolvedEscaper {
    fn _resolver(&self) -> &MetricsName {
        self.config.resolver()
    }

    fn _dependent_escaper(&self) -> Option<BTreeSet<MetricsName>> {
        let mut set = BTreeSet::new();
        for escaper in self.next_table.keys() {
            set.insert(escaper.clone());
        }
        Some(set)
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        AnyEscaperConfig::RouteResolved(self.config.clone())
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
        RouteResolvedEscaper::prepare_reload(config, stats)
    }

    async fn _check_out_next_escaper(
        &self,
        _task_notes: &ServerTaskNotes,
        upstream: &UpstreamAddr,
    ) -> Option<ArcEscaper> {
        if let Ok(escaper) = self.select_next(upstream).await {
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
        Err(TcpConnectError::EscaperNotUsable)
    }

    async fn _new_https_forward_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        _task_notes: &'a ServerTaskNotes,
        _task_stats: ArcHttpForwardTaskRemoteStats,
        _tls_config: &'a OpensslTlsClientConfig,
        _tls_name: &'a str,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::EscaperNotUsable)
    }

    async fn _new_ftp_control_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        _task_notes: &'a ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteHttpConnection, TcpConnectError> {
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
    ) -> Result<BoxFtpRemoteHttpConnection, TcpConnectError> {
        transfer_tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }
}
