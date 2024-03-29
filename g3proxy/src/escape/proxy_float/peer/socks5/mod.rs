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

use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;

use ahash::AHashMap;
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use slog::Logger;
use tokio::time::Instant;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::auth::{Password, Username};
use g3_types::net::{
    EgressArea, EgressInfo, Host, OpensslClientConfig, SocksAuth, TcpSockSpeedLimitConfig,
    UdpSockSpeedLimitConfig,
};

use super::{
    ArcNextProxyPeer, NextProxyPeer, NextProxyPeerInternal, ProxyFloatEscaperConfig,
    ProxyFloatEscaperStats,
};
use crate::module::http_forward::{ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectResult, TcpConnectTaskNotes};
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectResult, UdpConnectTaskNotes,
};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupResult, UdpRelayTaskNotes,
};
use crate::serve::ServerTaskNotes;

mod http_forward;
mod socks5_connect;
mod tcp_connect;
mod udp_connect;
mod udp_relay;

#[derive(Clone)]
struct ProxyFloatSocks5PeerSharedConfig {
    tcp_sock_speed_limit: TcpSockSpeedLimitConfig,
    expire_datetime: Option<DateTime<Utc>>,
    expire_instant: Option<Instant>,
    auth_info: SocksAuth,
}

impl Default for ProxyFloatSocks5PeerSharedConfig {
    fn default() -> Self {
        ProxyFloatSocks5PeerSharedConfig {
            tcp_sock_speed_limit: Default::default(),
            expire_datetime: None,
            expire_instant: None,
            auth_info: SocksAuth::None,
        }
    }
}

impl ProxyFloatSocks5PeerSharedConfig {
    fn set_user(&mut self, username: &Username, password: &Password) {
        self.auth_info = SocksAuth::User(username.clone(), password.clone());
    }
}

pub(super) struct ProxyFloatSocks5Peer {
    escaper_config: Arc<ProxyFloatEscaperConfig>,
    escaper_stats: Arc<ProxyFloatEscaperStats>,
    escape_logger: Logger,
    addr: SocketAddr,
    username: Username,
    password: Password,
    egress_info: EgressInfo,
    shared_config: Arc<ProxyFloatSocks5PeerSharedConfig>,
    transmute_udp_peer_ip: Option<AHashMap<IpAddr, IpAddr>>,
    udp_sock_speed_limit: UdpSockSpeedLimitConfig,
}

impl ProxyFloatSocks5Peer {
    pub(super) fn new_obj(
        escaper_config: Arc<ProxyFloatEscaperConfig>,
        escaper_stats: Arc<ProxyFloatEscaperStats>,
        escape_logger: Logger,
        addr: SocketAddr,
    ) -> ArcNextProxyPeer {
        Arc::new(ProxyFloatSocks5Peer {
            escaper_config,
            escaper_stats,
            escape_logger,
            addr,
            username: Username::empty(),
            password: Password::empty(),
            egress_info: Default::default(),
            shared_config: Arc::new(Default::default()),
            transmute_udp_peer_ip: None,
            udp_sock_speed_limit: Default::default(),
        })
    }

    pub(crate) fn transmute_udp_peer_addr(
        &self,
        returned_addr: SocketAddr,
        tcp_peer_ip: IpAddr,
    ) -> SocketAddr {
        if let Some(map) = &self.transmute_udp_peer_ip {
            let ip = map.get(&returned_addr.ip()).copied().unwrap_or(tcp_peer_ip);
            SocketAddr::new(ip, returned_addr.port())
        } else if returned_addr.ip().is_unspecified() {
            SocketAddr::new(tcp_peer_ip, returned_addr.port())
        } else {
            returned_addr
        }
    }
}

impl NextProxyPeerInternal for ProxyFloatSocks5Peer {
    fn set_isp(&mut self, isp: String) {
        self.egress_info.isp = Some(isp);
    }

    fn set_eip(&mut self, eip: IpAddr) {
        self.egress_info.ip = Some(eip);
    }

    fn set_area(&mut self, area: EgressArea) {
        self.egress_info.area = Some(area);
    }

    fn set_expire(&mut self, expire_datetime: DateTime<Utc>, expire_instant: Instant) {
        let shared_config = Arc::make_mut(&mut self.shared_config);
        shared_config.expire_datetime = Some(expire_datetime);
        shared_config.expire_instant = Some(expire_instant);
    }

    fn set_tcp_sock_speed_limit(&mut self, speed_limit: TcpSockSpeedLimitConfig) {
        let shared_config = Arc::make_mut(&mut self.shared_config);
        shared_config.tcp_sock_speed_limit = speed_limit;
    }

    fn set_kv(&mut self, k: &str, v: &Value) -> anyhow::Result<()> {
        match k {
            "username" => {
                self.username = g3_json::value::as_username(v)
                    .context(format!("invalid username value for key {k}"))?;
                Ok(())
            }
            "password" => {
                self.password = g3_json::value::as_password(v)
                    .context(format!("invalid password value for key {k}"))?;
                Ok(())
            }
            "transmute_udp_peer_ip" => {
                if let Value::Object(_) = v {
                    let map = g3_json::value::as_hashmap(
                        v,
                        |k| {
                            IpAddr::from_str(k)
                                .map_err(|e| anyhow!("the key {k} is not a valid ip address: {e}"))
                        },
                        g3_json::value::as_ipaddr,
                    )
                    .context(format!("invalid IP:IP hashmap value for key {k}"))?;
                    self.transmute_udp_peer_ip = Some(map.into_iter().collect::<AHashMap<_, _>>());
                } else {
                    let enable = g3_json::value::as_bool(v)?;
                    if enable {
                        self.transmute_udp_peer_ip = Some(AHashMap::default());
                    }
                }
                Ok(())
            }
            "udp_sock_speed_limit" => {
                self.udp_sock_speed_limit = g3_json::value::as_udp_sock_speed_limit(v)?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn finalize(&mut self) -> anyhow::Result<()> {
        let shared_config = Arc::make_mut(&mut self.shared_config);
        if !self.username.is_empty() {
            shared_config.set_user(&self.username, &self.password);
        }
        Ok(())
    }

    #[inline]
    fn expire_instant(&self) -> Option<Instant> {
        self.shared_config.expire_instant
    }

    #[inline]
    fn escaper_stats(&self) -> &Arc<ProxyFloatEscaperStats> {
        &self.escaper_stats
    }
}

#[async_trait]
impl NextProxyPeer for ProxyFloatSocks5Peer {
    async fn tcp_setup_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        self.socks5_new_tcp_connection(tcp_notes, task_notes, task_stats)
            .await
    }

    async fn tls_setup_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        tls_config: &'a OpensslClientConfig,
        tls_name: &'a Host,
    ) -> TcpConnectResult {
        self.socks5_new_tls_connection(tcp_notes, task_notes, task_stats, tls_config, tls_name)
            .await
    }

    async fn new_http_forward_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.http_forward_new_connection(tcp_notes, task_notes, task_stats)
            .await
    }

    async fn new_https_forward_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
        tls_config: &'a OpensslClientConfig,
        tls_name: &'a Host,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.https_forward_new_connection(tcp_notes, task_notes, task_stats, tls_config, tls_name)
            .await
    }

    async fn udp_setup_connection<'a>(
        &'a self,
        udp_notes: &'a mut UdpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult {
        self.udp_connect_to(udp_notes, task_notes, task_stats).await
    }

    async fn udp_setup_relay<'a>(
        &'a self,
        udp_notes: &'a mut UdpRelayTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        self.udp_setup_relay(udp_notes, task_notes, task_stats)
            .await
    }
}
