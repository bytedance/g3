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

use ahash::AHashMap;
use anyhow::anyhow;
use async_trait::async_trait;
use ip_network_table::IpNetworkTable;
use radix_trie::Trie;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::net::{Host, OpensslTlsClientConfig, UpstreamAddr};

use super::{ArcEscaper, Escaper, EscaperInternal, RouteEscaperStats};
use crate::config::escaper::route_upstream::RouteUpstreamEscaperConfig;
use crate::config::escaper::{AnyEscaperConfig, EscaperConfig};
use crate::module::ftp_over_http::{
    AnyFtpConnectContextParam, ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats,
    BoxFtpConnectContext, BoxFtpRemoteHttpConnection,
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
use crate::serve::ServerTaskNotes;

pub(super) struct RouteUpstreamEscaper {
    config: RouteUpstreamEscaperConfig,
    stats: Arc<RouteEscaperStats>,
    next_table: BTreeMap<String, ArcEscaper>,
    exact_match_ipaddr: AHashMap<IpAddr, ArcEscaper>,
    subnet_match_ipaddr: IpNetworkTable<ArcEscaper>,
    exact_match_domain: AHashMap<String, ArcEscaper>,
    do_child_match: bool,
    child_match_domain: Trie<String, ArcEscaper>,
    do_radix_match: bool,
    radix_match_domain: Trie<String, ArcEscaper>,
    default_next: ArcEscaper,
}

impl RouteUpstreamEscaper {
    fn new_obj(
        config: RouteUpstreamEscaperConfig,
        stats: Arc<RouteEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        let mut next_table = BTreeMap::new();
        if let Some(escapers) = config.dependent_escaper() {
            for escaper in escapers {
                let next = super::registry::get_or_insert_default(&escaper);
                next_table.insert(escaper, next);
            }
        }

        let default_next = Arc::clone(next_table.get(&config.default_next).unwrap());

        let mut exact_match_ipaddr = AHashMap::new();
        for (escaper, ips) in &config.exact_match_ipaddr {
            let next = &next_table.get(escaper).unwrap();
            for ip in ips {
                exact_match_ipaddr.insert(*ip, Arc::clone(next));
            }
        }
        let mut exact_match_domain = AHashMap::new();
        for (escaper, hosts) in &config.exact_match_domain {
            for host in hosts {
                let next = &next_table.get(escaper).unwrap();
                exact_match_domain.insert(host.to_string(), Arc::clone(next));
            }
        }

        let mut subnet_match_ipaddr = IpNetworkTable::new();
        for (escaper, subnets) in &config.subnet_match_ipaddr {
            for subnet in subnets {
                let next = &next_table.get(escaper).unwrap();
                subnet_match_ipaddr.insert(*subnet, Arc::clone(next));
            }
        }

        let do_child_match = !config.child_match_domain.is_empty();
        let mut child_match_domain = Trie::new();
        for (escaper, domains) in &config.child_match_domain {
            for domain in domains {
                let next = &next_table.get(escaper).unwrap();
                let reversed = g3_types::resolve::reverse_idna_domain(domain);
                child_match_domain.insert(reversed, Arc::clone(next));
            }
        }

        let do_radix_match = !config.radix_match_domain.is_empty();
        let mut radix_match_domain = Trie::new();
        for (escaper, domains) in &config.radix_match_domain {
            for domain in domains {
                let next = &next_table.get(escaper).unwrap();
                let reversed = domain.chars().rev().collect();
                radix_match_domain.insert(reversed, Arc::clone(next));
            }
        }

        let escaper = RouteUpstreamEscaper {
            config,
            stats,
            next_table,
            exact_match_ipaddr,
            subnet_match_ipaddr,
            exact_match_domain,
            do_child_match,
            child_match_domain,
            do_radix_match,
            radix_match_domain,
            default_next,
        };

        Ok(Arc::new(escaper))
    }

    pub(super) fn prepare_initial(config: AnyEscaperConfig) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::RouteUpstream(config) = config {
            let stats = Arc::new(RouteEscaperStats::new(config.name()));
            RouteUpstreamEscaper::new_obj(config, stats)
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<RouteEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::RouteUpstream(config) = config {
            RouteUpstreamEscaper::new_obj(config, stats)
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn select_next_by_ip(&self, ip: IpAddr) -> ArcEscaper {
        if !self.exact_match_ipaddr.is_empty() {
            if let Some(escaper) = self.exact_match_ipaddr.get(&ip) {
                return Arc::clone(escaper);
            }
        }

        if !self.subnet_match_ipaddr.is_empty() {
            if let Some((_, escaper)) = self.subnet_match_ipaddr.longest_match(ip) {
                return Arc::clone(escaper);
            }
        }

        Arc::clone(&self.default_next)
    }

    fn select_next_by_domain(&self, host: &str) -> ArcEscaper {
        if !self.exact_match_domain.is_empty() {
            if let Some(escaper) = self.exact_match_domain.get(host) {
                return Arc::clone(escaper);
            }
        }

        if self.do_child_match {
            let key = g3_types::resolve::reverse_idna_domain(host);
            if let Some(escaper) = self.child_match_domain.get_ancestor_value(&key) {
                return Arc::clone(escaper);
            }
        }

        if self.do_radix_match {
            let key: String = host.chars().rev().collect();
            if let Some(escaper) = self.radix_match_domain.get_ancestor_value(&key) {
                return Arc::clone(escaper);
            }
        }

        Arc::clone(&self.default_next)
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
    fn name(&self) -> &str {
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
        let escaper = self.select_next(&tcp_notes.upstream);
        self.stats.add_request_passed();
        escaper
            .tcp_setup_connection(tcp_notes, task_notes, task_stats)
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
        tcp_notes.escaper.clone_from(&self.config.name);
        let escaper = self.select_next(&tcp_notes.upstream);
        self.stats.add_request_passed();
        escaper
            .tls_setup_connection(tcp_notes, task_notes, task_stats, tls_config, tls_name)
            .await
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
        let escaper = self.select_next(upstream);
        self.stats.add_request_passed();
        escaper
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
        let escaper = self.select_next(&udp_notes.initial_peer);
        self.stats.add_request_passed();
        escaper
            .udp_setup_relay(udp_notes, task_notes, task_stats)
            .await
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
        let escaper = self.select_next(upstream);
        self.stats.add_request_passed();
        escaper
            .new_ftp_connect_context(Arc::clone(&escaper), task_notes, upstream)
            .await
    }
}

#[async_trait]
impl EscaperInternal for RouteUpstreamEscaper {
    fn _resolver(&self) -> &str {
        ""
    }

    fn _dependent_escaper(&self) -> Option<BTreeSet<String>> {
        let mut set = BTreeSet::new();
        for escaper in self.next_table.keys() {
            set.insert(escaper.to_string());
        }
        Some(set)
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        AnyEscaperConfig::RouteUpstream(self.config.clone())
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
        RouteUpstreamEscaper::prepare_reload(config, stats)
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

#[cfg(test)]
mod tests {

    #[test]
    fn join_vec_str_to_string() {
        let mut v = vec!["bc", "de"];
        v.insert(0, "a");
        v.push("d");
        assert_eq!(v.join("\n"), "a\nbc\nde\nd");
    }
}
