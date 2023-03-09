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

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use slog::Logger;
use tokio::time::Instant;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::net::{EgressArea, OpensslTlsClientConfig, TcpSockSpeedLimitConfig};

use super::{ProxyFloatEscaperConfig, ProxyFloatEscaperStats};
use crate::auth::UserUpstreamTrafficStats;
use crate::escape::EscaperStats;
use crate::module::http_forward::{ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectResult, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

mod http;
mod https;
mod socks5;

const CONFIG_KEY_PEER_TYPE: &str = "type";
const CONFIG_KEY_PEER_ADDR: &str = "addr";
const CONFIG_KEY_PEER_EXPIRE: &str = "expire";
const CONFIG_KEY_PEER_ISP: &str = "isp";
const CONFIG_KEY_PEER_EIP: &str = "eip";
const CONFIG_KEY_PEER_AREA: &str = "area";
const CONFIG_KEY_PEER_TCP_SOCK_SPEED_LIMIT: &str = "tcp_sock_speed_limit";

pub(super) trait NextProxyPeerInternal {
    fn set_isp(&mut self, isp: String);
    fn set_eip(&mut self, eip: IpAddr);
    fn set_area(&mut self, area: EgressArea);
    fn set_expire(&mut self, expire_datetime: DateTime<Utc>, expire_instant: Instant);
    fn set_tcp_sock_speed_limit(&mut self, speed_limit: TcpSockSpeedLimitConfig);
    fn set_kv(&mut self, k: &str, v: &Value) -> anyhow::Result<()>;
    fn finalize(&mut self) -> anyhow::Result<()>;

    fn expire_instant(&self) -> Option<Instant>;
    fn escaper_stats(&self) -> &Arc<ProxyFloatEscaperStats>;

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
    fn fetch_user_upstream_io_stats(
        &self,
        task_notes: &ServerTaskNotes,
    ) -> Vec<Arc<UserUpstreamTrafficStats>> {
        task_notes
            .user_ctx()
            .map(|ctx| {
                let escaper_stats = self.escaper_stats();
                ctx.fetch_upstream_traffic_stats(escaper_stats.name(), escaper_stats.extra_tags())
            })
            .unwrap_or_default()
    }
}

#[async_trait]
pub(super) trait NextProxyPeer: NextProxyPeerInternal {
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

    async fn new_http_forward_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError>;

    async fn new_https_forward_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
        tls_config: &'a OpensslTlsClientConfig,
        tls_name: &'a str,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError>;
}

pub(super) type ArcNextProxyPeer = Arc<dyn NextProxyPeer + Send + Sync>;

pub(super) fn parse_peers(
    escaper_config: &Arc<ProxyFloatEscaperConfig>,
    escaper_stats: &Arc<ProxyFloatEscaperStats>,
    escape_logger: Logger,
    records: &[Value],
    tls_config: &Option<Arc<OpensslTlsClientConfig>>,
) -> anyhow::Result<Vec<ArcNextProxyPeer>> {
    let mut peers = Vec::<ArcNextProxyPeer>::new();

    let instant_now = Instant::now();
    let datetime_now = Utc::now();

    'next_record: for record in records.iter() {
        if let Value::Object(map) = record {
            let peer_type = g3_json::get_required_str(map, CONFIG_KEY_PEER_TYPE)?;
            let addr_str = g3_json::get_required_str(map, CONFIG_KEY_PEER_ADDR)?;
            let addr = SocketAddr::from_str(addr_str)
                .map_err(|e| anyhow!("invalid peer addr {addr_str}: {e}"))?;
            let mut peer = match peer_type {
                "http" => http::ProxyFloatHttpPeer::new_obj(
                    Arc::clone(escaper_config),
                    Arc::clone(escaper_stats),
                    escape_logger.clone(),
                    addr,
                ),
                "https" => {
                    if let Some(tls_config) = tls_config {
                        https::ProxyFloatHttpsPeer::new_obj(
                            Arc::clone(escaper_config),
                            Arc::clone(escaper_stats),
                            escape_logger.clone(),
                            addr,
                            tls_config.clone(),
                        )
                    } else {
                        continue;
                    }
                }
                "socks5" => socks5::ProxyFloatSocks5Peer::new_obj(
                    Arc::clone(escaper_config),
                    Arc::clone(escaper_stats),
                    escape_logger.clone(),
                    addr,
                ),
                _ => return Err(anyhow!("unsupported peer type {peer_type}")),
            };
            let peer_mut = Arc::get_mut(&mut peer).unwrap();
            for (k, v) in map {
                match g3_json::key::normalize(k).as_str() {
                    CONFIG_KEY_PEER_TYPE | CONFIG_KEY_PEER_ADDR => {}
                    CONFIG_KEY_PEER_ISP => {
                        if let Ok(isp) = g3_json::value::as_string(v) {
                            peer_mut.set_isp(isp);
                        }
                        // not a required field, skip if value format is invalid
                    }
                    CONFIG_KEY_PEER_EIP => {
                        if let Ok(ip) = g3_json::value::as_ipaddr(v) {
                            peer_mut.set_eip(ip);
                        }
                        // not a required field, skip if value format is invalid
                    }
                    CONFIG_KEY_PEER_AREA => {
                        if let Ok(area) = g3_json::value::as_egress_area(v) {
                            peer_mut.set_area(area);
                        }
                        // not a required field, skip if value format is invalid
                    }
                    CONFIG_KEY_PEER_EXPIRE => {
                        let datetime_expire_orig = g3_json::value::as_rfc3339_datetime(v)?;
                        let datetime_expire = match datetime_expire_orig
                            .checked_sub_signed(escaper_config.expire_guard_duration)
                        {
                            Some(datetime) => datetime,
                            None => continue 'next_record,
                        };

                        if datetime_expire <= datetime_now {
                            continue 'next_record;
                        }

                        if let Ok(duration) =
                            datetime_expire.signed_duration_since(datetime_now).to_std()
                        {
                            if let Some(instant_expire) = instant_now.checked_add(duration) {
                                peer_mut.set_expire(datetime_expire_orig, instant_expire);
                            } else {
                                continue 'next_record;
                            }
                        } else {
                            continue 'next_record;
                        }
                    }
                    CONFIG_KEY_PEER_TCP_SOCK_SPEED_LIMIT => {
                        let limit = g3_json::value::as_tcp_sock_speed_limit(v)?;
                        peer_mut.set_tcp_sock_speed_limit(limit);
                    }
                    _ => peer_mut
                        .set_kv(k, v)
                        .context(format!("failed to parse key {k}"))?,
                }
            }
            peer_mut.finalize()?;
            peers.push(peer);
        } else {
            return Err(anyhow!("record root type should be json map"));
        }
    }
    Ok(peers)
}
