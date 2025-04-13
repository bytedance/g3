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

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::Arc;

use ahash::AHashMap;
use anyhow::anyhow;
use async_trait::async_trait;
use ip_network_table::IpNetworkTable;
use radix_trie::{Trie, TrieCommon};
use regex::RegexSet;
use rustc_hash::FxHashMap;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::metrics::NodeName;
use g3_types::net::{Host, UpstreamAddr};

use super::{ArcEscaper, Escaper, EscaperInternal, EscaperRegistry, RouteEscaperStats};
use crate::audit::AuditContext;
use crate::config::escaper::route_upstream::RouteUpstreamEscaperConfig;
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
    exact_match_ipaddr: FxHashMap<IpAddr, ArcEscaper>,
    subnet_match_ipaddr: IpNetworkTable<ArcEscaper>,
    exact_match_domain: AHashMap<Arc<str>, ArcEscaper>,
    do_child_match: bool,
    child_match_domain: Trie<String, ArcEscaper>,
    do_radix_match: bool,
    radix_match_domain: Trie<String, ArcEscaper>,
    do_regex_match: bool,
    regex_match_domain: Trie<String, Vec<(RegexSet, ArcEscaper)>>,
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

        let mut exact_match_ipaddr = FxHashMap::default();
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
                exact_match_domain.insert(host.clone(), Arc::clone(next));
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

        let do_regex_match = !config.regex_match_domain.is_empty();
        let mut regex_match_map: BTreeMap<String, Vec<(RegexSet, ArcEscaper)>> = BTreeMap::new();
        for (escaper, rules) in &config.regex_match_domain {
            let mut parent_regex_map: BTreeMap<String, Vec<&str>> = BTreeMap::new();
            for rule in rules {
                let parent_reversed = g3_types::resolve::reverse_idna_domain(&rule.parent_domain);
                parent_regex_map
                    .entry(parent_reversed)
                    .or_default()
                    .push(&rule.sub_domain_regex);
            }

            let next = next_table.get(escaper).unwrap();
            for (parent_domain, regexes) in parent_regex_map {
                let regex_set = RegexSet::new(regexes).unwrap();
                regex_match_map
                    .entry(parent_domain)
                    .or_default()
                    .push((regex_set, next.clone()));
            }
        }
        let mut regex_match_domain = Trie::new();
        for (parent_domain, value) in regex_match_map {
            regex_match_domain.insert(parent_domain, value);
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
            do_regex_match,
            regex_match_domain,
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

        if self.do_regex_match {
            let key: String = g3_types::resolve::reverse_idna_domain(host);
            if let Some(sub_trie) = self.regex_match_domain.get_ancestor(&key) {
                if let Some(rules) = sub_trie.value() {
                    let prefix_len = sub_trie.prefix().as_bytes().len();
                    let sub = &key.as_str()[..key.len() - prefix_len];
                    for (regex, escaper) in rules {
                        if regex.is_match(sub) {
                            return Arc::clone(escaper);
                        }
                    }
                }
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
