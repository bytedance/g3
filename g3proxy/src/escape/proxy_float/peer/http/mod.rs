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
use std::sync::Arc;

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use slog::Logger;
use tokio::time::Instant;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::auth::{Password, Username};
use g3_types::net::{EgressArea, EgressInfo, OpensslTlsClientConfig, TcpSockSpeedLimitConfig};

use super::{
    ArcNextProxyPeer, NextProxyPeer, NextProxyPeerInternal, ProxyFloatEscaperConfig,
    ProxyFloatEscaperStats,
};
use crate::module::http_forward::{ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectResult, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

mod http_connect;
mod http_forward;
mod tcp_connect;

#[derive(Clone, Default)]
struct ProxyFloatHttpPeerSharedConfig {
    tcp_conn_speed_limit: TcpSockSpeedLimitConfig,
    expire_datetime: Option<DateTime<Utc>>,
    expire_instant: Option<Instant>,
    append_http_headers: Vec<String>,
}

impl ProxyFloatHttpPeerSharedConfig {
    fn set_user(&mut self, username: &Username, password: &Password) {
        self.append_http_headers
            .push(g3_http::header::proxy_authorization_basic(
                username, password,
            ));
    }

    fn set_header(&mut self, name: &str, value: &str) {
        self.append_http_headers
            .push(format!("{name}: {value}\r\n"));
    }
}

pub(super) struct ProxyFloatHttpPeer {
    escaper_config: Arc<ProxyFloatEscaperConfig>,
    escaper_stats: Arc<ProxyFloatEscaperStats>,
    escape_logger: Logger,
    addr: SocketAddr,
    username: Username,
    password: Password,
    egress_info: EgressInfo,
    http_connect_rsp_hdr_max_size: usize,
    shared_config: Arc<ProxyFloatHttpPeerSharedConfig>,
}

impl ProxyFloatHttpPeer {
    pub(super) fn new_obj(
        escaper_config: Arc<ProxyFloatEscaperConfig>,
        escaper_stats: Arc<ProxyFloatEscaperStats>,
        escape_logger: Logger,
        addr: SocketAddr,
    ) -> ArcNextProxyPeer {
        Arc::new(ProxyFloatHttpPeer {
            escaper_config,
            escaper_stats,
            escape_logger,
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
        shared_config.tcp_conn_speed_limit = speed_limit;
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

    #[inline]
    fn escaper_stats(&self) -> &Arc<ProxyFloatEscaperStats> {
        &self.escaper_stats
    }
}

#[async_trait]
impl NextProxyPeer for ProxyFloatHttpPeer {
    async fn tcp_setup_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        self.http_connect_new_tcp_connection(tcp_notes, task_notes, task_stats)
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
        self.http_connect_new_tls_connection(
            tcp_notes, task_notes, task_stats, tls_config, tls_name,
        )
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
        tls_config: &'a OpensslTlsClientConfig,
        tls_name: &'a str,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.https_forward_new_connection(tcp_notes, task_notes, task_stats, tls_config, tls_name)
            .await
    }
}
