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

use anyhow::{Context, anyhow};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustc_hash::FxHashMap;
use serde_json::Value;
use tokio::time::Instant;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::auth::{Password, Username};
use g3_types::net::{EgressInfo, Host, TcpSockSpeedLimitConfig, UdpSockSpeedLimitConfig};

use super::socks5::ProxyFloatSocks5PeerSharedConfig;
use super::{ArcNextProxyPeer, NextProxyPeer, NextProxyPeerInternal, ProxyFloatEscaper};
use crate::module::http_forward::{ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection};
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

mod http_forward;
mod socks5_connect;
mod udp_connect;
mod udp_relay;

pub(super) struct ProxyFloatSocks5sPeer {
    addr: SocketAddr,
    tls_name: Host,
    username: Username,
    password: Password,
    egress_info: EgressInfo,
    shared_config: Arc<ProxyFloatSocks5PeerSharedConfig>,
    transmute_udp_peer_ip: Option<FxHashMap<IpAddr, IpAddr>>,
    udp_sock_speed_limit: UdpSockSpeedLimitConfig,
    end_on_control_closed: bool,
}

impl ProxyFloatSocks5sPeer {
    pub(super) fn new_obj(addr: SocketAddr) -> ArcNextProxyPeer {
        Arc::new(ProxyFloatSocks5sPeer {
            addr,
            tls_name: Host::Ip(addr.ip()),
            username: Username::empty(),
            password: Password::empty(),
            egress_info: Default::default(),
            shared_config: Arc::new(Default::default()),
            transmute_udp_peer_ip: None,
            udp_sock_speed_limit: Default::default(),
            end_on_control_closed: false,
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

impl NextProxyPeerInternal for ProxyFloatSocks5sPeer {
    fn egress_info_mut(&mut self) -> &mut EgressInfo {
        &mut self.egress_info
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
            "tls_name" => {
                self.tls_name = g3_json::value::as_host(v)
                    .context(format!("invalid tls server name value for key {k}"))?;
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
                    self.transmute_udp_peer_ip = Some(map.into_iter().collect::<FxHashMap<_, _>>());
                } else {
                    let enable = g3_json::value::as_bool(v)?;
                    if enable {
                        self.transmute_udp_peer_ip = Some(FxHashMap::default());
                    }
                }
                Ok(())
            }
            "udp_sock_speed_limit" => {
                self.udp_sock_speed_limit = g3_json::value::as_udp_sock_speed_limit(v)?;
                Ok(())
            }
            "end_on_control_closed" => {
                self.end_on_control_closed = g3_json::value::as_bool(v)?;
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
}

#[async_trait]
impl NextProxyPeer for ProxyFloatSocks5sPeer {
    #[inline]
    fn peer_addr(&self) -> SocketAddr {
        self.addr
    }

    #[inline]
    fn tcp_sock_speed_limit(&self) -> &TcpSockSpeedLimitConfig {
        &self.shared_config.tcp_sock_speed_limit
    }

    #[inline]
    fn expire_datetime(&self) -> Option<DateTime<Utc>> {
        self.shared_config.expire_datetime
    }

    #[inline]
    fn egress_info(&self) -> EgressInfo {
        self.egress_info.clone()
    }

    async fn tcp_setup_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        self.socks5_new_tcp_connection(escaper, task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn tls_setup_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        self.socks5_new_tls_connection(escaper, task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn new_http_forward_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.http_forward_new_connection(escaper, task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn new_https_forward_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.https_forward_new_connection(escaper, task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn udp_setup_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &UdpConnectTaskConf<'_>,
        udp_notes: &mut UdpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult {
        udp_notes.expire = self.expire_datetime();
        self.udp_connect_to(escaper, task_conf, udp_notes, task_notes, task_stats)
            .await
    }

    async fn udp_setup_relay(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &UdpRelayTaskConf<'_>,
        udp_notes: &mut UdpRelayTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        udp_notes.expire = self.expire_datetime();
        self.udp_setup_relay(escaper, task_conf, task_notes, task_stats)
            .await
    }
}
