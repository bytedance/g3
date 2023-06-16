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
use std::net::IpAddr;
use std::sync::Arc;

use async_trait::async_trait;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::collection::{SelectiveHash, SelectiveItem, SelectivePickPolicy, SelectiveVec};
use g3_types::metrics::MetricsName;
use g3_types::net::{Host, HttpForwardCapability, OpensslTlsClientConfig, UpstreamAddr};

use crate::config::escaper::AnyEscaperConfig;
use crate::module::ftp_over_http::{
    AnyFtpConnectContextParam, ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats,
    BoxFtpConnectContext, BoxFtpRemoteConnection,
};
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, BoxHttpForwardContext,
};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectResult, TcpConnectTaskNotes};
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectResult, UdpConnectTaskNotes,
};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupResult, UdpRelayTaskNotes,
};
use crate::serve::ServerTaskNotes;

mod registry;
pub(crate) use registry::{foreach as foreach_escaper, get_names, get_or_insert_default};

mod stats;
pub(crate) use stats::{
    ArcEscaperInternalStats, ArcEscaperStats, EscaperForbiddenSnapshot, EscaperForbiddenStats,
    EscaperInterfaceStats, EscaperInternalStats, EscaperStats, EscaperTcpStats, EscaperUdpStats,
    RouteEscaperSnapshot, RouteEscaperStats,
};

mod direct_fixed;
mod direct_float;
mod dummy_deny;
mod proxy_float;
mod proxy_http;
mod proxy_https;
mod proxy_socks5;
mod route_client;
mod route_failover;
mod route_mapping;
mod route_query;
mod route_resolved;
mod route_select;
mod route_upstream;
mod trick_float;

mod ops;
pub use ops::load_all;
pub(crate) use ops::{get_escaper, reload, update_dependency_to_resolver};

/// Functions in this trait should only be called from registry module,
/// as Escaper and its reload notifier should be locked together.
/// If not locked, there may be reload notify during getting Escaper and
/// its notifier, which will lead to missing of the notification.
#[async_trait]
pub(crate) trait EscaperInternal {
    fn _resolver(&self) -> &MetricsName;
    fn _dependent_escaper(&self) -> Option<BTreeSet<MetricsName>>;

    fn _clone_config(&self) -> AnyEscaperConfig;
    fn _update_config_in_place(&self, flags: u64, config: AnyEscaperConfig) -> anyhow::Result<()>;

    /// registry lock is allowed in this method
    async fn _lock_safe_reload(&self, config: AnyEscaperConfig) -> anyhow::Result<ArcEscaper>;

    fn _local_http_forward_capability(&self) -> HttpForwardCapability {
        HttpForwardCapability::default()
    }

    async fn _check_out_next_escaper(
        &self,
        task_notes: &ServerTaskNotes,
        upstream: &UpstreamAddr,
    ) -> Option<ArcEscaper>;

    async fn _new_http_forward_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError>;

    async fn _new_https_forward_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
        tls_config: &'a OpensslTlsClientConfig,
        tls_name: &'a str,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError>;

    async fn _new_ftp_control_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError>;
    async fn _new_ftp_transfer_connection<'a>(
        &'a self,
        transfer_tcp_notes: &'a mut TcpConnectTaskNotes,
        control_tcp_notes: &'a TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteTransferStats,
        context: AnyFtpConnectContextParam,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError>;

    fn _trick_float_weight(&self) -> u8 {
        0
    }
}

#[async_trait]
pub(crate) trait Escaper: EscaperInternal {
    fn name(&self) -> &MetricsName;
    fn escaper_type(&self) -> &str;
    fn get_escape_stats(&self) -> Option<ArcEscaperStats> {
        None
    }
    fn ref_route_stats(&self) -> Option<&Arc<RouteEscaperStats>> {
        None
    }

    async fn publish(&self, data: String) -> anyhow::Result<()>;

    async fn tcp_setup_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult;

    async fn tls_setup_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        tls_config: &'a OpensslTlsClientConfig,
        tls_name: &'a str,
    ) -> TcpConnectResult;

    async fn udp_setup_connection<'a>(
        &'a self,
        udp_notes: &'a mut UdpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult;

    async fn udp_setup_relay<'a>(
        &'a self,
        udp_notes: &'a mut UdpRelayTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult;

    fn new_http_forward_context(&self, escaper: ArcEscaper) -> BoxHttpForwardContext;

    async fn new_ftp_connect_context<'a>(
        &'a self,
        escaper: ArcEscaper,
        task_notes: &'a ServerTaskNotes,
        upstream: &'a UpstreamAddr,
    ) -> BoxFtpConnectContext;
}

pub(crate) type ArcEscaper = Arc<dyn Escaper + Send + Sync>;

pub(crate) trait EscaperExt: Escaper {
    fn select_consistent<'a, T>(
        &'a self,
        nodes: &'a SelectiveVec<T>,
        pick_policy: SelectivePickPolicy,
        task_notes: &'a ServerTaskNotes,
        host: &'a Host,
    ) -> &'a T
    where
        T: SelectiveItem + SelectiveHash,
    {
        #[derive(Hash)]
        struct ConsistentKey<'a> {
            client_ip: IpAddr,
            user: Option<&'a str>,
            host: &'a Host,
        }

        match pick_policy {
            SelectivePickPolicy::Random => nodes.pick_random(),
            SelectivePickPolicy::Serial => nodes.pick_serial(),
            SelectivePickPolicy::RoundRobin => nodes.pick_round_robin(),
            SelectivePickPolicy::Rendezvous => {
                let key = ConsistentKey {
                    client_ip: task_notes.client_addr.ip(),
                    user: task_notes.user_ctx().map(|c| c.user().name()),
                    host,
                };
                nodes.pick_rendezvous(&key)
            }
            SelectivePickPolicy::JumpHash => {
                let key = ConsistentKey {
                    client_ip: task_notes.client_addr.ip(),
                    user: task_notes.user_ctx().map(|c| c.user().name()),
                    host,
                };
                nodes.pick_jump(&key)
            }
        }
    }
}
