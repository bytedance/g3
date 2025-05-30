/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, anyhow};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::time::Instant;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::auth::{Password, Username};
use g3_types::net::{EgressInfo, TcpSockSpeedLimitConfig};

use super::{
    ArcNextProxyPeer, NextProxyPeer, NextProxyPeerInternal, ProxyFloatEscaper,
    ProxyFloatEscaperStats,
};
use crate::module::http_forward::{ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection};
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectResult, TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectError, UdpConnectResult, UdpConnectTaskConf,
    UdpConnectTaskNotes,
};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupError, UdpRelaySetupResult, UdpRelayTaskConf,
    UdpRelayTaskNotes,
};
use crate::serve::ServerTaskNotes;

mod http_connect;
mod http_forward;

pub(crate) use http_forward::HttpPeerHttpForwardReader;

#[derive(Clone, Default)]
pub(crate) struct ProxyFloatHttpPeerSharedConfig {
    pub(crate) tcp_sock_speed_limit: TcpSockSpeedLimitConfig,
    pub(crate) expire_datetime: Option<DateTime<Utc>>,
    pub(crate) expire_instant: Option<Instant>,
    pub(crate) append_http_headers: Vec<String>,
}

impl ProxyFloatHttpPeerSharedConfig {
    pub(crate) fn set_user(&mut self, username: &Username, password: &Password) {
        self.append_http_headers
            .push(g3_http::header::proxy_authorization_basic(
                username, password,
            ));
    }

    pub(crate) fn set_header(&mut self, name: &str, value: &str) {
        self.append_http_headers
            .push(format!("{name}: {value}\r\n"));
    }
}

pub(super) struct ProxyFloatHttpPeer {
    addr: SocketAddr,
    username: Username,
    password: Password,
    egress_info: EgressInfo,
    http_connect_rsp_hdr_max_size: usize,
    shared_config: Arc<ProxyFloatHttpPeerSharedConfig>,
}

impl ProxyFloatHttpPeer {
    pub(super) fn new_obj(addr: SocketAddr) -> ArcNextProxyPeer {
        Arc::new(ProxyFloatHttpPeer {
            addr,
            username: Username::empty(),
            password: Password::empty(),
            egress_info: Default::default(),
            http_connect_rsp_hdr_max_size: 4096,
            shared_config: Arc::new(Default::default()),
        })
    }
}

impl NextProxyPeerInternal for ProxyFloatHttpPeer {
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
            "http_connect_rsp_header_max_size" => {
                self.http_connect_rsp_hdr_max_size = g3_json::humanize::as_usize(v)?;
                Ok(())
            }
            "extra_append_headers" => {
                if let Value::Object(map) = v {
                    let shared_config = Arc::make_mut(&mut self.shared_config);
                    for (name, value) in map {
                        let value = g3_json::value::as_ascii(value).context(format!(
                            "invalid ascii string value for extra header {name}"
                        ))?;
                        shared_config.set_header(name, value.as_str());
                    }
                    Ok(())
                } else {
                    Err(anyhow!("invalid map value for key {k}"))
                }
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
impl NextProxyPeer for ProxyFloatHttpPeer {
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
        self.http_connect_new_tcp_connection(escaper, task_conf, tcp_notes, task_notes, task_stats)
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
        self.http_connect_new_tls_connection(escaper, task_conf, tcp_notes, task_notes, task_stats)
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
        _escaper: &ProxyFloatEscaper,
        _task_conf: &UdpConnectTaskConf<'_>,
        _udp_notes: &mut UdpConnectTaskNotes,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult {
        Err(UdpConnectError::MethodUnavailable)
    }

    async fn udp_setup_relay(
        &self,
        _escaper: &ProxyFloatEscaper,
        _task_conf: &UdpRelayTaskConf<'_>,
        _udp_notes: &mut UdpRelayTaskNotes,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        Err(UdpRelaySetupError::MethodUnavailable)
    }
}
