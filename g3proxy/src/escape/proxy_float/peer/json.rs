/*
 * Copyright 2024 ByteDance and/or its affiliates.
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
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, anyhow};
use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::time::Instant;

use super::{
    ArcNextProxyPeer, CONFIG_KEY_PEER_ADDR, CONFIG_KEY_PEER_AREA, CONFIG_KEY_PEER_EIP,
    CONFIG_KEY_PEER_EXPIRE, CONFIG_KEY_PEER_ID, CONFIG_KEY_PEER_ISP,
    CONFIG_KEY_PEER_TCP_SOCK_SPEED_LIMIT, CONFIG_KEY_PEER_TYPE,
};
use crate::config::escaper::proxy_float::ProxyFloatEscaperConfig;

pub(super) fn do_parse_peer(
    value: &Value,
    escaper_config: &ProxyFloatEscaperConfig,
    instant_now: Instant,
    datetime_now: DateTime<Utc>,
) -> anyhow::Result<Option<(String, ArcNextProxyPeer)>> {
    if let Value::Object(map) = value {
        let peer_type = g3_json::get_required_str(map, CONFIG_KEY_PEER_TYPE)?;
        let addr_str = g3_json::get_required_str(map, CONFIG_KEY_PEER_ADDR)?;
        let addr = SocketAddr::from_str(addr_str)
            .map_err(|e| anyhow!("invalid peer addr {addr_str}: {e}"))?;
        let mut peer = match peer_type {
            "http" => super::http::ProxyFloatHttpPeer::new_obj(addr),
            "https" => super::https::ProxyFloatHttpsPeer::new_obj(addr),
            "socks5" => super::socks5::ProxyFloatSocks5Peer::new_obj(addr),
            "socks5s" | "socks5+tls" => super::socks5s::ProxyFloatSocks5sPeer::new_obj(addr),
            _ => return Err(anyhow!("unsupported peer type {peer_type}")),
        };
        let mut peer_id = String::new();
        let peer_mut = Arc::get_mut(&mut peer).unwrap();
        for (k, v) in map {
            match g3_json::key::normalize(k).as_str() {
                CONFIG_KEY_PEER_TYPE | CONFIG_KEY_PEER_ADDR => {}
                CONFIG_KEY_PEER_ID => {
                    peer_id = g3_json::value::as_string(v)?;
                }
                CONFIG_KEY_PEER_ISP => {
                    if let Ok(isp) = g3_json::value::as_string(v) {
                        peer_mut.egress_info_mut().set_isp(isp);
                    }
                    // not a required field, skip if value format is invalid
                }
                CONFIG_KEY_PEER_EIP => {
                    if let Ok(ip) = g3_json::value::as_ipaddr(v) {
                        peer_mut.egress_info_mut().set_ip(ip);
                    }
                    // not a required field, skip if value format is invalid
                }
                CONFIG_KEY_PEER_AREA => {
                    if let Ok(area) = g3_json::value::as_egress_area(v) {
                        peer_mut.egress_info_mut().set_area(area);
                    }
                    // not a required field, skip if value format is invalid
                }
                CONFIG_KEY_PEER_EXPIRE => {
                    let datetime_expire_orig = g3_json::value::as_rfc3339_datetime(v)?;
                    let Some(datetime_expire) = datetime_expire_orig
                        .checked_sub_signed(escaper_config.expire_guard_duration)
                    else {
                        return Ok(None);
                    };
                    if datetime_expire <= datetime_now {
                        return Ok(None);
                    }
                    let Ok(duration) = datetime_expire.signed_duration_since(datetime_now).to_std()
                    else {
                        return Ok(None);
                    };
                    let Some(instant_expire) = instant_now.checked_add(duration) else {
                        return Ok(None);
                    };
                    peer_mut.set_expire(datetime_expire_orig, instant_expire);
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
        Ok(Some((peer_id, peer)))
    } else {
        Err(anyhow!("record root type should be json map"))
    }
}
