/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::metrics::NodeName;
use g3_types::net::{Host, UpstreamAddr};

use super::{ArcEscaper, Escaper, EscaperInternal, EscaperRegistry, RouteEscaperStats};
use crate::audit::AuditContext;
use crate::config::escaper::route_upstream::{
    ChildMatch, ExactMatch, RegexMatch, RouteUpstreamEscaperConfig, SubnetMatch, SuffixMatch,
};
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

pub(super) struct RouteUpstreamEscaper {
    config: RouteUpstreamEscaperConfig,
    stats: Arc<RouteEscaperStats>,
    next_table: BTreeMap<NodeName, ArcEscaper>,
    exact_match: ExactMatch<ArcEscaper>,
    subnet_match: Option<SubnetMatch<ArcEscaper>>,
    child_match: Option<ChildMatch<ArcEscaper>>,
    suffix_match: Option<SuffixMatch<ArcEscaper>>,
    regex_match: Option<RegexMatch<ArcEscaper>>,
    default_next: ArcEscaper,
}

impl RouteUpstreamEscaper {
    fn new_obj<F>(
        config: RouteUpstreamEscaperConfig,
        stats: Arc<RouteEscaperStats>,
        mut fetch_escaper: F,
    ) -> anyhow::Result<ArcEscaper>
    where
        F: FnMut(&NodeName) -> ArcEscaper,
    {
        let mut next_table = BTreeMap::new();
        if let Some(escapers) = config.dependent_escaper() {
            for escaper in escapers {
                let next = fetch_escaper(&escaper);
                next_table.insert(escaper, next);
            }
        }

        let default_next = Arc::clone(next_table.get(&config.default_next).unwrap());

        let exact_match = config.exact_match.build(&next_table);
        let subnet_match = config.subnet_match.build(&next_table);
        let child_match = config.child_match.build(&next_table);
        let suffix_match = config.suffix_match.build(&next_table);
        let regex_match = config.regex_match.build(&next_table);

        let escaper = RouteUpstreamEscaper {
            config,
            stats,
            next_table,
            exact_match,
            subnet_match,
            child_match,
            suffix_match,
            regex_match,
            default_next,
        };

        Ok(Arc::new(escaper))
    }

    pub(super) fn prepare_initial(
        config: RouteUpstreamEscaperConfig,
    ) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::new(RouteEscaperStats::new(config.name()));
        RouteUpstreamEscaper::new_obj(config, stats, super::registry::get_or_insert_default)
    }

    fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<RouteEscaperStats>,
        registry: &mut EscaperRegistry,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::RouteUpstream(config) = config {
            RouteUpstreamEscaper::new_obj(config, stats, |name| {
                registry.get_or_insert_default(name)
            })
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn select_next_by_ip(&self, ip: IpAddr) -> ArcEscaper {
        if let Some(escaper) = self.exact_match.check_ip(ip) {
            return escaper.clone();
        }
        if let Some(subnet_match) = &self.subnet_match {
            if let Some(escaper) = subnet_match.check_ip(ip) {
                return escaper.clone();
            }
        }
        self.default_next.clone()
    }

    fn select_next_by_domain(&self, host: &str) -> ArcEscaper {
        if let Some(escaper) = self.exact_match.check_domain(host) {
            return escaper.clone();
        }
        if let Some(child_match) = &self.child_match {
            if let Some(escaper) = child_match.check_domain(host) {
                return escaper.clone();
            }
        }
        if let Some(suffix_match) = &self.suffix_match {
            if let Some(escaper) = suffix_match.check_domain(host) {
                return escaper.clone();
            }
        }
        if let Some(regex_match) = &self.regex_match {
            if let Some(escaper) = regex_match.check_domain(host) {
                return escaper.clone();
            }
        }
        self.default_next.clone()
    }

    fn select_next(&self, ups: &UpstreamAddr) -> ArcEscaper {
        match ups.host() {
            Host::Ip(ip) => self.select_next_by_ip(*ip),
            Host::Domain(domain) => self.select_next_by_domain(domain),
        }
    }
}

#[async_trait]
impl Escaper for RouteUpstreamEscaper {
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
        let escaper = self.select_next(task_conf.upstream);
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
        let escaper = self.select_next(task_conf.tcp.upstream);
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
        let escaper = self.select_next(task_conf.upstream);
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
        let escaper = self.select_next(task_conf.initial_peer);
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
        let escaper = self.select_next(task_conf.upstream);
        self.stats.add_request_passed();
        escaper
            .new_ftp_connect_context(Arc::clone(&escaper), task_conf, task_notes)
            .await
    }
}

#[async_trait]
impl EscaperInternal for RouteUpstreamEscaper {
    fn _resolver(&self) -> &NodeName {
        Default::default()
    }

    fn _depend_on_escaper(&self, name: &NodeName) -> bool {
        self.next_table.contains_key(name)
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        AnyEscaperConfig::RouteUpstream(self.config.clone())
    }

    fn _reload(
        &self,
        config: AnyEscaperConfig,
        registry: &mut EscaperRegistry,
    ) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::clone(&self.stats);
        RouteUpstreamEscaper::prepare_reload(config, stats, registry)
    }

    async fn _check_out_next_escaper(
        &self,
        _task_notes: &ServerTaskNotes,
        upstream: &UpstreamAddr,
    ) -> Option<ArcEscaper> {
        let escaper = self.select_next(upstream);
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
