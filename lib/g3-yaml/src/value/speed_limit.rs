/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::limit::{GlobalDatagramSpeedLimitConfig, GlobalStreamSpeedLimitConfig};
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

pub fn as_global_stream_speed_limit(v: &Yaml) -> anyhow::Result<GlobalStreamSpeedLimitConfig> {
    match v {
        Yaml::String(_) | Yaml::Integer(_) => {
            let limit = crate::humanize::as_u64(v).context("invalid humanize u64 value")?;
            Ok(GlobalStreamSpeedLimitConfig::per_second(limit))
        }
        Yaml::Hash(map) => {
            let mut config = GlobalStreamSpeedLimitConfig::default();
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "replenish_interval" => {
                    let interval = crate::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    config.set_replenish_interval(interval);
                    Ok(())
                }
                "replenish_bytes" => {
                    let size = crate::humanize::as_u64(v)
                        .context(format!("invalid humanize u64 value for key {k}"))?;
                    config.set_replenish_bytes(size);
                    Ok(())
                }
                "max_burst_bytes" => {
                    let size = crate::humanize::as_u64(v)
                        .context(format!("invalid humanize u64 value for key {k}"))?;
                    config.set_max_burst_bytes(size);
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
            config.check()?;
            Ok(config)
        }
        _ => Err(anyhow!("invalid yaml value type")),
    }
}

pub fn as_global_datagram_speed_limit(v: &Yaml) -> anyhow::Result<GlobalDatagramSpeedLimitConfig> {
    match v {
        Yaml::String(_) | Yaml::Integer(_) => {
            let limit = crate::humanize::as_u64(v).context("invalid humanize u64 value")?;
            Ok(GlobalDatagramSpeedLimitConfig::per_second(limit))
        }
        Yaml::Hash(map) => {
            let mut config = GlobalDatagramSpeedLimitConfig::default();
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "replenish_interval" => {
                    let interval = crate::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    config.set_replenish_interval(interval);
                    Ok(())
                }
                "replenish_bytes" => {
                    let size = crate::humanize::as_u64(v)
                        .context(format!("invalid humanize u64 value for key {k}"))?;
                    config.set_replenish_bytes(size);
                    Ok(())
                }
                "replenish_packets" => {
                    let count = crate::humanize::as_u64(v)
                        .context(format!("invalid humanize u64 value for key {k}"))?;
                    config.set_replenish_packets(count);
                    Ok(())
                }
                "max_burst_bytes" => {
                    let size = crate::humanize::as_u64(v)
                        .context(format!("invalid humanize u64 value for key {k}"))?;
                    config.set_max_burst_bytes(size);
                    Ok(())
                }
                "max_burst_packets" => {
                    let size = crate::humanize::as_u64(v)
                        .context(format!("invalid humanize u64 value for key {k}"))?;
                    config.set_max_burst_packets(size);
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
            config.check()?;
            Ok(config)
        }
        _ => Err(anyhow!("invalid yaml value type")),
    }
}
