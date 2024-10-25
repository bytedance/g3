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

use std::net::SocketAddr;
use std::sync::Arc;

use ahash::AHashMap;
use anyhow::Context;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rand::seq::IteratorRandom;
use serde_json::Value;
use tokio::time::Instant;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::net::{EgressInfo, TcpSockSpeedLimitConfig};

use super::{ProxyFloatEscaper, ProxyFloatEscaperConfig, ProxyFloatEscaperStats};
use crate::module::http_forward::{ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection};
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectResult, TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectResult, UdpConnectTaskNotes,
};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupResult, UdpRelayTaskNotes,
};
use crate::serve::ServerTaskNotes;

mod json;

mod http;
mod https;
mod socks5;
mod socks5s;

const CONFIG_KEY_PEER_TYPE: &str = "type";
const CONFIG_KEY_PEER_ID: &str = "id";
const CONFIG_KEY_PEER_ADDR: &str = "addr";
const CONFIG_KEY_PEER_EXPIRE: &str = "expire";
const CONFIG_KEY_PEER_ISP: &str = "isp";
const CONFIG_KEY_PEER_EIP: &str = "eip";
const CONFIG_KEY_PEER_AREA: &str = "area";
const CONFIG_KEY_PEER_TCP_SOCK_SPEED_LIMIT: &str = "tcp_sock_speed_limit";

pub(super) trait NextProxyPeerInternal {
    fn egress_info_mut(&mut self) -> &mut EgressInfo;
    fn set_expire(&mut self, expire_datetime: DateTime<Utc>, expire_instant: Instant);
    fn set_tcp_sock_speed_limit(&mut self, speed_limit: TcpSockSpeedLimitConfig);
    fn set_kv(&mut self, k: &str, v: &Value) -> anyhow::Result<()>;
    fn finalize(&mut self) -> anyhow::Result<()>;

    fn expire_instant(&self) -> Option<Instant>;

    fn is_expired(&self) -> bool {
        if let Some(expire) = self.expire_instant() {
            expire.checked_duration_since(Instant::now()).is_none()
        } else {
            false
        }
    }
    fn expected_alive_minutes(&self) -> u64 {
        if let Some(expire) = self.expire_instant() {
            expire
                .checked_duration_since(Instant::now())
                .map(|d| d.as_secs() / 60)
                .unwrap_or(0)
        } else {
            u64::MAX
        }
    }
}

#[async_trait]
pub(super) trait NextProxyPeer: NextProxyPeerInternal {
    fn peer_addr(&self) -> SocketAddr;
    fn tcp_sock_speed_limit(&self) -> &TcpSockSpeedLimitConfig;
    fn expire_datetime(&self) -> Option<DateTime<Utc>>;
    fn egress_info(&self) -> EgressInfo;

    async fn tcp_setup_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult;

    async fn tls_setup_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult;

    async fn new_http_forward_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError>;

    async fn new_https_forward_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError>;

    async fn udp_setup_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        udp_notes: &mut UdpConnectTaskNotes,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult;

    async fn udp_setup_relay(
        &self,
        escaper: &ProxyFloatEscaper,
        udp_notes: &mut UdpRelayTaskNotes,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult;
}

pub(super) type ArcNextProxyPeer = Arc<dyn NextProxyPeer + Send + Sync>;

pub(super) fn parse_peer(
    escaper_config: &Arc<ProxyFloatEscaperConfig>,
    record: &Value,
) -> anyhow::Result<Option<ArcNextProxyPeer>> {
    let instant_now = Instant::now();
    let datetime_now = Utc::now();

    json::do_parse_peer(record, escaper_config, instant_now, datetime_now).map(|r| r.map(|v| v.1))
}

pub(super) fn parse_peers(
    escaper_config: &Arc<ProxyFloatEscaperConfig>,
    records: &[Value],
) -> anyhow::Result<PeerSet> {
    let mut peer_set = PeerSet::default();

    let instant_now = Instant::now();
    let datetime_now = Utc::now();

    for (i, record) in records.iter().enumerate() {
        if let Some((peer_id, peer)) =
            json::do_parse_peer(record, escaper_config, instant_now, datetime_now)
                .context(format!("invalid value for record #{i}"))?
        {
            if peer_id.is_empty() {
                peer_set.push_unnamed(peer);
            } else {
                peer_set.insert_named(peer_id, peer);
            }
        }
    }
    Ok(peer_set)
}

#[derive(Default)]
pub(super) struct PeerSet {
    unnamed: Vec<ArcNextProxyPeer>,
    named: AHashMap<String, ArcNextProxyPeer>,
}

impl PeerSet {
    fn push_unnamed(&mut self, peer: ArcNextProxyPeer) {
        self.unnamed.push(peer);
    }

    fn insert_named(&mut self, id: String, peer: ArcNextProxyPeer) {
        self.named.insert(id, peer);
    }

    pub(super) fn select_random_peer(&self) -> Option<ArcNextProxyPeer> {
        self.unnamed
            .iter()
            .chain(self.named.values())
            .filter(|p| !p.is_expired())
            .choose(&mut rand::thread_rng())
            .cloned()
    }

    pub(super) fn select_stable_peer(&self) -> Option<&ArcNextProxyPeer> {
        if self.unnamed.len() == 1 {
            return self.unnamed.first();
        }
        if self.named.len() == 1 {
            return self.named.values().next();
        }
        None
    }

    #[inline]
    pub(super) fn select_named_peer(&self, id: &str) -> Option<ArcNextProxyPeer> {
        self.named.get(id).cloned()
    }
}
