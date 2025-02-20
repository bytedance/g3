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

use anyhow::{Context, anyhow};
use serde_json::Value;

use g3_types::limit::{GlobalDatagramSpeedLimitConfig, GlobalStreamSpeedLimitConfig};
use g3_types::net::{TcpSockSpeedLimitConfig, UdpSockSpeedLimitConfig};

pub fn as_tcp_sock_speed_limit(v: &Value) -> anyhow::Result<TcpSockSpeedLimitConfig> {
    let mut config = TcpSockSpeedLimitConfig::default();
    match v {
        Value::String(_) | Value::Number(_) => {
            let limit = crate::humanize::as_usize(v).context("invalid humanize usize value")?;
            config.shift_millis = g3_types::net::RATE_LIMIT_SHIFT_MILLIS_DEFAULT;
            config.max_north = limit;
            config.max_south = limit;
        }
        Value::Object(map) => {
            for (k, v) in map {
                match crate::key::normalize(k).as_str() {
                    "shift" | "shift_millis" => {
                        config.shift_millis = crate::value::as_u8(v)
                            .context(format!("invalid u8 value for key {k}"))?;
                    }
                    "upload" | "north" | "upload_bytes" | "north_bytes" => {
                        config.max_north = crate::humanize::as_usize(v)
                            .context(format!("invalid humanize usize value for key {k}"))?;
                    }
                    "download" | "south" | "download_bytes" | "south_bytes" => {
                        config.max_south = crate::humanize::as_usize(v)
                            .context(format!("invalid humanize usize value for key {k}"))?;
                    }
                    _ => return Err(anyhow!("invalid key {k}")),
                }
            }
        }
        _ => return Err(anyhow!("invalid json value type")),
    }
    config.validate()?;
    Ok(config)
}

pub fn as_udp_sock_speed_limit(v: &Value) -> anyhow::Result<UdpSockSpeedLimitConfig> {
    let mut config = UdpSockSpeedLimitConfig::default();
    match v {
        Value::String(_) | Value::Number(_) => {
            let limit = crate::humanize::as_usize(v).context("invalid humanize usize value")?;
            config.shift_millis = g3_types::net::RATE_LIMIT_SHIFT_MILLIS_DEFAULT;
            config.max_north_bytes = limit;
            config.max_south_bytes = limit;
        }
        Value::Object(map) => {
            for (k, v) in map {
                match crate::key::normalize(k).as_str() {
                    "shift" | "shift_millis" => {
                        config.shift_millis = crate::value::as_u8(v)
                            .context(format!("invalid u8 value for key {k}"))?;
                    }
                    "upload_packets" | "north_packets" => {
                        config.max_north_packets = crate::value::as_usize(v)
                            .context(format!("invalid usize value for key {k}"))?;
                    }
                    "download_packets" | "south_packets" => {
                        config.max_south_packets = crate::value::as_usize(v)
                            .context(format!("invalid humanize usize value for key {k}"))?;
                    }
                    "upload_bytes" | "north_bytes" => {
                        config.max_north_bytes = crate::humanize::as_usize(v)
                            .context(format!("invalid humanize usize value for key {k}"))?;
                    }
                    "download_bytes" | "south_bytes" => {
                        config.max_south_bytes = crate::humanize::as_usize(v)
                            .context(format!("invalid humanize usize value for key {k}"))?;
                    }
                    _ => return Err(anyhow!("invalid key {k}")),
                }
            }
        }
        _ => return Err(anyhow!("invalid json value type")),
    }
    config.validate()?;
    Ok(config)
}

pub fn as_global_stream_speed_limit(v: &Value) -> anyhow::Result<GlobalStreamSpeedLimitConfig> {
    match v {
        Value::String(_) | Value::Number(_) => {
            let limit = crate::humanize::as_u64(v).context("invalid humanize usize value")?;
            Ok(GlobalStreamSpeedLimitConfig::per_second(limit))
        }
        Value::Object(map) => {
            let mut config = GlobalStreamSpeedLimitConfig::default();
            for (k, v) in map {
                match crate::key::normalize(k).as_str() {
                    "replenish_interval" => {
                        let interval = crate::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_replenish_interval(interval);
                    }
                    "replenish_bytes" => {
                        let size = crate::humanize::as_u64(v)
                            .context(format!("invalid humanize u64 value for key {k}"))?;
                        config.set_replenish_bytes(size);
                    }
                    "max_burst_bytes" => {
                        let size = crate::humanize::as_u64(v)
                            .context(format!("invalid humanize u64 value for key {k}"))?;
                        config.set_max_burst_bytes(size);
                    }
                    _ => return Err(anyhow!("invalid key {k}")),
                }
            }
            config.check()?;
            Ok(config)
        }
        _ => Err(anyhow!("invalid json value type")),
    }
}

pub fn as_global_datagram_speed_limit(v: &Value) -> anyhow::Result<GlobalDatagramSpeedLimitConfig> {
    match v {
        Value::String(_) | Value::Number(_) => {
            let limit = crate::humanize::as_u64(v).context("invalid humanize u64 value")?;
            Ok(GlobalDatagramSpeedLimitConfig::per_second(limit))
        }
        Value::Object(map) => {
            let mut config = GlobalDatagramSpeedLimitConfig::default();
            for (k, v) in map {
                match crate::key::normalize(k).as_str() {
                    "replenish_interval" => {
                        let interval = crate::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_replenish_interval(interval);
                    }
                    "replenish_bytes" => {
                        let size = crate::humanize::as_u64(v)
                            .context(format!("invalid humanize u64 value for key {k}"))?;
                        config.set_replenish_bytes(size);
                    }
                    "replenish_packets" => {
                        let count = crate::humanize::as_u64(v)
                            .context(format!("invalid humanize u64 value for key {k}"))?;
                        config.set_replenish_packets(count);
                    }
                    "max_burst_bytes" => {
                        let size = crate::humanize::as_u64(v)
                            .context(format!("invalid humanize u64 value for key {k}"))?;
                        config.set_max_burst_bytes(size);
                    }
                    "max_burst_packets" => {
                        let count = crate::humanize::as_u64(v)
                            .context(format!("invalid humanize u64 value for key {k}"))?;
                        config.set_max_burst_packets(count);
                    }
                    _ => return Err(anyhow!("invalid key {k}")),
                }
            }
            config.check()?;
            Ok(config)
        }
        _ => Err(anyhow!("invalid json value type")),
    }
}
