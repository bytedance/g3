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
use g3_types::collection::{SelectiveItem, SelectivePickPolicy, SelectiveVec};
use g3_types::metrics::NodeName;
use g3_types::net::{Host, HttpForwardCapability, UpstreamAddr};

use crate::audit::AuditContext;
use crate::config::escaper::AnyEscaperConfig;
use crate::module::ftp_over_http::{
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpConnectContext,
    BoxFtpRemoteConnection,
};
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, BoxHttpForwardContext,
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

mod registry;
use registry::EscaperRegistry;
pub(crate) use registry::{foreach as foreach_escaper, get_names, get_or_insert_default};

mod stats;
pub(crate) use stats::{
    ArcEscaperInternalStats, ArcEscaperStats, EscaperForbiddenSnapshot, EscaperForbiddenStats,
    EscaperInterfaceStats, EscaperInternalStats, EscaperStats, EscaperTcpConnectSnapshot,
    EscaperTcpStats, EscaperTlsSnapshot, EscaperTlsStats, EscaperUdpStats, RouteEscaperSnapshot,
    RouteEscaperStats,
};

mod egress_path;
pub(crate) use egress_path::EgressPathSelection;

mod comply_audit;
mod direct_fixed;
mod direct_float;
mod divert_tcp;
mod dummy_deny;
mod proxy_float;
mod proxy_http;
mod proxy_https;
mod proxy_socks5;
mod proxy_socks5s;
mod route_client;
mod route_failover;
mod route_geoip;
mod route_mapping;
mod route_query;
mod route_resolved;
mod route_select;
mod route_upstream;
mod trick_float;

mod ops;
pub use ops::load_all;
pub(crate) use ops::{
    get_escaper, reload, update_dependency_to_auditor, update_dependency_to_resolver,
};

/// Functions in this trait should only be called from registry module,
/// as Escaper and its reload notifier should be locked together.
/// If not locked, there may be reload notify during getting Escaper and
/// its notifier, which will lead to missing of the notification.
#[async_trait]
pub(crate) trait EscaperInternal {
    fn _resolver(&self) -> &NodeName;
    fn _auditor(&self) -> Option<&NodeName> {
        None
    }
    fn _dependent_escaper(&self) -> Option<BTreeSet<NodeName>>;

    fn _clone_config(&self) -> AnyEscaperConfig;

    fn _reload(
        &self,
        config: AnyEscaperConfig,
        registry: &mut EscaperRegistry,
    ) -> anyhow::Result<ArcEscaper>;

    fn _clean_to_offline(&self) {}

    fn _local_http_forward_capability(&self) -> HttpForwardCapability {
        HttpForwardCapability::default()
    }

    async fn _check_out_next_escaper(
        &self,
        _task_notes: &ServerTaskNotes,
        _upstream: &UpstreamAddr,
    ) -> Option<ArcEscaper> {
        None
    }
    fn _update_audit_context(&self, _audit_ctx: &mut AuditContext) {}

    async fn _new_http_forward_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError>;

    async fn _new_https_forward_connection(
        &self,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError>;

    async fn _new_ftp_control_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError>;
    async fn _new_ftp_transfer_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        transfer_tcp_notes: &mut TcpConnectTaskNotes,
        control_tcp_notes: &TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteTransferStats,
        ftp_server: &UpstreamAddr,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError>;

    fn _trick_float_weight(&self) -> u8 {
        0
    }
}

#[async_trait]
pub(crate) trait Escaper: EscaperInternal {
    fn name(&self) -> &NodeName;
    #[allow(unused)]
    fn escaper_type(&self) -> &str;
    fn get_escape_stats(&self) -> Option<ArcEscaperStats> {
        None
    }
    fn ref_route_stats(&self) -> Option<&Arc<RouteEscaperStats>> {
        None
    }

    async fn publish(&self, data: String) -> anyhow::Result<()>;

    async fn tcp_setup_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        audit_ctx: &mut AuditContext,
    ) -> TcpConnectResult;

    async fn tls_setup_connection(
        &self,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        audit_ctx: &mut AuditContext,
    ) -> TcpConnectResult;

    async fn udp_setup_connection(
        &self,
        task_conf: &UdpConnectTaskConf<'_>,
        udp_notes: &mut UdpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult;

    async fn udp_setup_relay(
        &self,
        task_conf: &UdpRelayTaskConf<'_>,
        udp_notes: &mut UdpRelayTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult;

    fn new_http_forward_context(&self, escaper: ArcEscaper) -> BoxHttpForwardContext;

    async fn new_ftp_connect_context(
        &self,
        escaper: ArcEscaper,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
    ) -> BoxFtpConnectContext;
}

pub(crate) type ArcEscaper = Arc<dyn Escaper + Send + Sync>;

pub(crate) trait EscaperExt: Escaper {
    fn select_consistent<'a, 'b, T>(
        &'a self,
        nodes: &'b SelectiveVec<T>,
        pick_policy: SelectivePickPolicy,
        task_notes: &'a ServerTaskNotes,
        host: &'a Host,
    ) -> &'b T
    where
        T: SelectiveItem,
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
            SelectivePickPolicy::Ketama => {
                let key = ConsistentKey {
                    client_ip: task_notes.client_ip(),
                    user: task_notes.raw_user_name().map(|s| s.as_ref()),
                    host,
                };
                nodes.pick_ketama(&key)
            }
            SelectivePickPolicy::Rendezvous => {
                let key = ConsistentKey {
                    client_ip: task_notes.client_ip(),
                    user: task_notes.raw_user_name().map(|s| s.as_ref()),
                    host,
                };
                nodes.pick_rendezvous(&key)
            }
            SelectivePickPolicy::JumpHash => {
                let key = ConsistentKey {
                    client_ip: task_notes.client_ip(),
                    user: task_notes.raw_user_name().map(|s| s.as_ref()),
                    host,
                };
                nodes.pick_jump(&key)
            }
        }
    }
}
