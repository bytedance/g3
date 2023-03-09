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

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::net::{OpensslTlsClientConfig, UpstreamAddr};

use super::{ArcEscaper, Escaper, EscaperInternal, RouteEscaperStats};
use crate::config::escaper::route_client::RouteClientEscaperConfig;
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
    ArcUdpConnectTaskRemoteStats, UdpConnectResult, UdpConnectTaskNotes,
};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupResult, UdpRelayTaskNotes,
};
use crate::serve::ServerTaskNotes;

pub(super) struct RouteClientEscaper {
    config: RouteClientEscaperConfig,
    stats: Arc<RouteEscaperStats>,
    next_table: BTreeMap<String, ArcEscaper>,
    exact_match_ipaddr: AHashMap<IpAddr, ArcEscaper>,
    subnet_match_ipaddr: IpNetworkTable<ArcEscaper>,
    default_next: ArcEscaper,
}

impl RouteClientEscaper {
    fn new_obj(
        config: RouteClientEscaperConfig,
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
            let next = next_table.get(escaper).unwrap();
            for ip in ips {
                exact_match_ipaddr.insert(*ip, Arc::clone(next));
            }
        }

        let mut subnet_match_ipaddr = IpNetworkTable::new();
        for (escaper, subnets) in &config.subnet_match_ipaddr {
            for subnet in subnets {
                let next = next_table.get(escaper).unwrap();
                subnet_match_ipaddr.insert(*subnet, Arc::clone(next));
            }
        }

        let escaper = RouteClientEscaper {
            config,
            stats,
            next_table,
            exact_match_ipaddr,
            subnet_match_ipaddr,
            default_next,
        };

        Ok(Arc::new(escaper))
    }

    pub(super) fn prepare_initial(config: AnyEscaperConfig) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::RouteClient(config) = config {
            let stats = Arc::new(RouteEscaperStats::new(config.name()));
            RouteClientEscaper::new_obj(config, stats)
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<RouteEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::RouteClient(config) = config {
            RouteClientEscaper::new_obj(config, stats)
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn select_next(&self, ip: IpAddr) -> ArcEscaper {
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
}

#[async_trait]
impl Escaper for RouteClientEscaper {
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
        let escaper = self.select_next(task_notes.client_addr.ip());
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
        let escaper = self.select_next(task_notes.client_addr.ip());
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
        let escaper = self.select_next(task_notes.client_addr.ip());
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
        let escaper = self.select_next(task_notes.client_addr.ip());
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
        let escaper = self.select_next(task_notes.client_addr.ip());
        self.stats.add_request_passed();
        escaper
            .new_ftp_connect_context(Arc::clone(&escaper), task_notes, upstream)
            .await
    }
}

#[async_trait]
impl EscaperInternal for RouteClientEscaper {
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
        AnyEscaperConfig::RouteClient(self.config.clone())
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
        RouteClientEscaper::prepare_reload(config, stats)
    }

    async fn _check_out_next_escaper(
        &self,
        task_notes: &ServerTaskNotes,
        _upstream: &UpstreamAddr,
    ) -> Option<ArcEscaper> {
        let escaper = self.select_next(task_notes.client_addr.ip());
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
