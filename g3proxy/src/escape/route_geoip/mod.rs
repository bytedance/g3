/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use fixedbitset::FixedBitSet;
use fnv::FnvHashMap;
use ip_network_table::IpNetworkTable;
use rustc_hash::FxHashMap;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_geoip_types::{ContinentCode, IpLocation, IsoCountryCode};
use g3_ip_locate::IpLocationServiceHandle;
use g3_resolver::ResolveError;
use g3_types::metrics::NodeName;
use g3_types::net::{Host, UpstreamAddr};

use super::{ArcEscaper, Escaper, EscaperInternal, EscaperRegistry, RouteEscaperStats};
use crate::audit::AuditContext;
use crate::config::escaper::route_geoip::RouteGeoIpEscaperConfig;
use crate::config::escaper::{AnyEscaperConfig, EscaperConfig};
use crate::module::ftp_over_http::{
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpConnectContext,
    BoxFtpRemoteConnection, DenyFtpConnectContext,
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
use crate::resolve::{ArcIntegratedResolverHandle, HappyEyeballsResolveJob};
use crate::serve::ServerTaskNotes;

pub(super) struct RouteGeoIpEscaper {
    config: RouteGeoIpEscaperConfig,
    stats: Arc<RouteEscaperStats>,
    resolver_handle: ArcIntegratedResolverHandle,
    ip_locate_handle: IpLocationServiceHandle,
    next_table: BTreeMap<NodeName, ArcEscaper>,
    lpm_table: IpNetworkTable<ArcEscaper>,
    asn_table: FxHashMap<u32, ArcEscaper>,
    country_bitset: FixedBitSet,
    country_table: FnvHashMap<u16, ArcEscaper>,
    continent_bitset: FixedBitSet,
    continent_table: FnvHashMap<u8, ArcEscaper>,
    default_next: ArcEscaper,
    check_ip_location: bool,
}

impl RouteGeoIpEscaper {
    fn new_obj<F>(
        config: RouteGeoIpEscaperConfig,
        stats: Arc<RouteEscaperStats>,
        mut fetch_escaper: F,
    ) -> anyhow::Result<ArcEscaper>
    where
        F: FnMut(&NodeName) -> ArcEscaper,
    {
        let resolver_handle = crate::resolve::get_handle(config.resolver())?;
        let ip_locate_handle = config.ip_locate_service.spawn_ip_locate_agent()?;

        let mut next_table = BTreeMap::new();
        if let Some(escapers) = config.dependent_escaper() {
            for escaper in escapers {
                let next = fetch_escaper(&escaper);
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

        let mut asn_table = FxHashMap::default();
        for (escaper, asn_set) in &config.asn_rules {
            let next = next_table.get(escaper).unwrap();
            for asn in asn_set {
                asn_table.insert(*asn, Arc::clone(next));
            }
        }

        let mut country_bitset = FixedBitSet::with_capacity(IsoCountryCode::variant_count());
        let mut country_table = FnvHashMap::default();
        for (escaper, countries) in &config.country_rules {
            let next = next_table.get(escaper).unwrap();
            for country in countries {
                country_bitset.set(*country as usize, true);
                country_table.insert(*country as u16, Arc::clone(next));
            }
        }

        let mut continent_bitset = FixedBitSet::with_capacity(ContinentCode::variant_count());
        let mut continent_table = FnvHashMap::default();
        for (escaper, continents) in &config.continent_rules {
            let next = next_table.get(escaper).unwrap();
            for continent in continents {
                continent_bitset.set(*continent as usize, true);
                continent_table.insert(*continent as u8, Arc::clone(next));
            }
        }

        let check_asn_db = !asn_table.is_empty();
        let check_country_db = !(country_bitset.is_empty() && country_bitset.is_empty());
        let check_ip_location = check_asn_db || check_country_db;
        let escaper = RouteGeoIpEscaper {
            config,
            stats,
            resolver_handle,
            ip_locate_handle,
            next_table,
            lpm_table,
            asn_table,
            country_bitset,
            country_table,
            continent_bitset,
            continent_table,
            default_next,
            check_ip_location,
        };

        Ok(Arc::new(escaper))
    }

    pub(super) fn prepare_initial(config: RouteGeoIpEscaperConfig) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::new(RouteEscaperStats::new(config.name()));
        RouteGeoIpEscaper::new_obj(config, stats, super::registry::get_or_insert_default)
    }

    fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<RouteEscaperStats>,
        registry: &mut EscaperRegistry,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::RouteGeoIp(config) = config {
            RouteGeoIpEscaper::new_obj(config, stats, |name| registry.get_or_insert_default(name))
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
                    domain.clone(),
                )?;
                let v = resolver_job
                    .get_r1_or_first(self.config.resolution_delay, usize::MAX)
                    .await?;
                self.config
                    .resolve_strategy
                    .pick_best(v)
                    .ok_or(ResolveError::UnexpectedError(
                        "resolver job return ok but with no ip can be selected",
                    ))
            }
        }
    }

    fn select_next_by_ip_location(&self, location: &IpLocation) -> Option<ArcEscaper> {
        if !self.asn_table.is_empty() {
            if let Some(asn) = location.network_asn() {
                if let Some(escaper) = self.asn_table.get(&asn) {
                    return Some(Arc::clone(escaper));
                }
            }
        }

        if let Some(country) = location.country() {
            if self.country_bitset.contains(country as usize) {
                if let Some(escaper) = self.country_table.get(&(country as u16)) {
                    return Some(Arc::clone(escaper));
                }
            }
        }

        if let Some(continent) = location.continent() {
            if self.continent_bitset.contains(continent as usize) {
                if let Some(escaper) = self.continent_table.get(&(continent as u8)) {
                    return Some(Arc::clone(escaper));
                }
            }
        }

        None
    }

    async fn select_next_by_ip(&self, ip: IpAddr) -> ArcEscaper {
        if !self.lpm_table.is_empty() {
            if let Some((_net, escaper)) = self.lpm_table.longest_match(ip) {
                return Arc::clone(escaper);
            }
        }

        if self.check_ip_location {
            if let Some(location) = self.ip_locate_handle.fetch(ip).await {
                if let Some(escaper) = self.select_next_by_ip_location(&location) {
                    return escaper;
                }
            }
        }

        Arc::clone(&self.default_next)
    }

    async fn select_next(&self, ups: &UpstreamAddr) -> Result<ArcEscaper, ResolveError> {
        let ip = self.get_upstream_ip(ups.host()).await?;

        let escaper = self.select_next_by_ip(ip).await;
        Ok(escaper)
    }
}

#[async_trait]
impl Escaper for RouteGeoIpEscaper {
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
        match self.select_next(task_conf.upstream).await {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .tcp_setup_connection(task_conf, tcp_notes, task_notes, task_stats, audit_ctx)
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Err(e.into())
            }
        }
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
        match self.select_next(task_conf.tcp.upstream).await {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .tls_setup_connection(task_conf, tcp_notes, task_notes, task_stats, audit_ctx)
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Err(e.into())
            }
        }
    }

    async fn udp_setup_connection(
        &self,
        task_conf: &UdpConnectTaskConf<'_>,
        udp_notes: &mut UdpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult {
        udp_notes.escaper.clone_from(&self.config.name);
        match self.select_next(task_conf.upstream).await {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .udp_setup_connection(task_conf, udp_notes, task_notes, task_stats)
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Err(e.into())
            }
        }
    }

    async fn udp_setup_relay(
        &self,
        task_conf: &UdpRelayTaskConf<'_>,
        udp_notes: &mut UdpRelayTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        udp_notes.escaper.clone_from(&self.config.name);
        match self.select_next(task_conf.initial_peer).await {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .udp_setup_relay(task_conf, udp_notes, task_notes, task_stats)
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

    async fn new_ftp_connect_context(
        &self,
        _escaper: ArcEscaper,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
    ) -> BoxFtpConnectContext {
        match self.select_next(task_conf.upstream).await {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .new_ftp_connect_context(Arc::clone(&escaper), task_conf, task_notes)
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
impl EscaperInternal for RouteGeoIpEscaper {
    fn _resolver(&self) -> &NodeName {
        self.config.resolver()
    }

    fn _depend_on_escaper(&self, name: &NodeName) -> bool {
        self.next_table.contains_key(name)
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        AnyEscaperConfig::RouteGeoIp(self.config.clone())
    }

    fn _reload(
        &self,
        config: AnyEscaperConfig,
        registry: &mut EscaperRegistry,
    ) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::clone(&self.stats);
        RouteGeoIpEscaper::prepare_reload(config, stats, registry)
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
