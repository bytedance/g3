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

use anyhow::{anyhow, Context};
use yaml_rust::Yaml;

use g3_types::net::{TcpSockSpeedLimitConfig, UdpSockSpeedLimitConfig};

pub fn as_tcp_sock_speed_limit(v: &Yaml) -> anyhow::Result<TcpSockSpeedLimitConfig> {
    let mut config = TcpSockSpeedLimitConfig::default();
    match v {
        Yaml::String(_) | Yaml::Integer(_) => {
            let limit = crate::humanize::as_usize(v).context("invalid humanize usize value")?;
            config.shift_millis = g3_types::net::RATE_LIMIT_SHIFT_MILLIS_DEFAULT;
            config.max_north = limit;
            config.max_south = limit;
        }
        Yaml::Hash(map) => {
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "shift" | "shift_millis" => {
                    config.shift_millis =
                        crate::value::as_u8(v).context(format!("invalid u8 value for key {k}"))?;
                    Ok(())
                }
                "upload" | "north" | "upload_bytes" | "north_bytes" => {
                    config.max_north = crate::humanize::as_usize(v)
                        .context(format!("invalid humanize usize value for key {k}"))?;
                    Ok(())
                }
                "download" | "south" | "download_bytes" | "south_bytes" => {
                    config.max_south = crate::humanize::as_usize(v)
                        .context(format!("invalid humanize usize value for key {k}"))?;
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
        }
        _ => return Err(anyhow!("invalid yaml value type")),
    }
    config.validate()?;
    Ok(config)
}

pub fn as_udp_sock_speed_limit(v: &Yaml) -> anyhow::Result<UdpSockSpeedLimitConfig> {
    let mut config = UdpSockSpeedLimitConfig::default();
    match v {
        Yaml::String(_) | Yaml::Integer(_) => {
            let limit = crate::humanize::as_usize(v).context("invalid humanize usize value")?;
            config.shift_millis = g3_types::net::RATE_LIMIT_SHIFT_MILLIS_DEFAULT;
            config.max_north_bytes = limit;
            config.max_south_bytes = limit;
        }
        Yaml::Hash(map) => {
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "shift" | "shift_millis" => {
                    config.shift_millis =
                        crate::value::as_u8(v).context(format!("invalid u8 value for key {k}"))?;
                    Ok(())
                }
                "upload_packets" | "north_packets" => {
                    config.max_north_packets = crate::value::as_usize(v)
                        .context(format!("invalid usize value for key {k}"))?;
                    Ok(())
                }
                "download_packets" | "south_packets" => {
                    config.max_south_packets = crate::value::as_usize(v)
                        .context(format!("invalid humanize usize value for key {k}"))?;
                    Ok(())
                }
                "upload_bytes" | "north_bytes" => {
                    config.max_north_bytes = crate::humanize::as_usize(v)
                        .context(format!("invalid humanize usize value for key {k}"))?;
                    Ok(())
                }
                "download_bytes" | "south_bytes" => {
                    config.max_south_bytes = crate::humanize::as_usize(v)
                        .context(format!("invalid humanize usize value for key {k}"))?;
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
        }
        _ => return Err(anyhow!("invalid yaml value type")),
    }
    config.validate()?;
    Ok(config)
}
