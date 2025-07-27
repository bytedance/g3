/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn as_tcp_sock_speed_limit_ok() {
        // String input
        let value = json!("10MB");
        let config = as_tcp_sock_speed_limit(&value).unwrap();
        assert_eq!(config.max_north, 10_000_000);
        assert_eq!(config.max_south, 10_000_000);
        assert_eq!(
            config.shift_millis,
            g3_types::net::RATE_LIMIT_SHIFT_MILLIS_DEFAULT
        );

        // Number input
        let value = json!(102400);
        let config = as_tcp_sock_speed_limit(&value).unwrap();
        assert_eq!(config.max_north, 102400);
        assert_eq!(config.max_south, 102400);

        // Object input
        let value = json!({
            "shift": 5,
            "upload": "5MB",
            "download": "10MB"
        });
        let config = as_tcp_sock_speed_limit(&value).unwrap();
        assert_eq!(config.shift_millis, 5);
        assert_eq!(config.max_north, 5_000_000);
        assert_eq!(config.max_south, 10_000_000);

        // Alternative keys
        let value = json!({
            "shift_millis": 10,
            "north_bytes": "1MB",
            "south_bytes": "2MB"
        });
        let config = as_tcp_sock_speed_limit(&value).unwrap();
        assert_eq!(config.shift_millis, 10);
        assert_eq!(config.max_north, 1_000_000);
        assert_eq!(config.max_south, 2_000_000);
    }

    #[test]
    fn as_tcp_sock_speed_limit_err() {
        // Invalid type (array)
        let value = json!([]);
        assert!(as_tcp_sock_speed_limit(&value).is_err());

        // Invalid key
        let value = json!({"invalid_key": 100});
        assert!(as_tcp_sock_speed_limit(&value).is_err());

        // Shift value too large
        let value = json!({
            "shift": 100,
            "upload": "1MB"
        });
        assert!(as_tcp_sock_speed_limit(&value).is_err());

        // Upload limit zero when shift is set
        let value = json!({
            "shift": 5,
            "upload": 0
        });
        assert!(as_tcp_sock_speed_limit(&value).is_err());

        // Invalid value type
        let value = json!({"shift": "abc"});
        assert!(as_tcp_sock_speed_limit(&value).is_err());
    }

    #[test]
    fn as_udp_sock_speed_limit_ok() {
        // String input
        let value = json!("5MB");
        let config = as_udp_sock_speed_limit(&value).unwrap();
        assert_eq!(config.max_north_bytes, 5_000_000);
        assert_eq!(config.max_south_bytes, 5_000_000);

        // Number input
        let value = json!(51200);
        let config = as_udp_sock_speed_limit(&value).unwrap();
        assert_eq!(config.max_north_bytes, 51200);
        assert_eq!(config.max_south_bytes, 51200);

        // Object input with packets and bytes
        let value = json!({
            "shift": 4,
            "upload_packets": 500,
            "download_packets": 1000,
            "upload_bytes": "2MB",
            "download_bytes": "4MB"
        });
        let config = as_udp_sock_speed_limit(&value).unwrap();
        assert_eq!(config.shift_millis, 4);
        assert_eq!(config.max_north_packets, 500);
        assert_eq!(config.max_south_packets, 1000);
        assert_eq!(config.max_north_bytes, 2_000_000);
        assert_eq!(config.max_south_bytes, 4_000_000);

        // Alternative keys
        let value = json!({
            "shift_millis": 10,
            "north_packets": 1000,
            "south_packets": 2000,
            "north_bytes": "1MB",
            "south_bytes": "2MB"
        });
        let config = as_udp_sock_speed_limit(&value).unwrap();
        assert_eq!(config.shift_millis, 10);
        assert_eq!(config.max_north_packets, 1000);
        assert_eq!(config.max_south_packets, 2000);
        assert_eq!(config.max_north_bytes, 1_000_000);
        assert_eq!(config.max_south_bytes, 2_000_000);
    }

    #[test]
    fn as_udp_sock_speed_limit_err() {
        // Invalid type (null)
        let value = json!(null);
        assert!(as_udp_sock_speed_limit(&value).is_err());

        // Invalid key
        let value = json!({"invalid_key": 100});
        assert!(as_udp_sock_speed_limit(&value).is_err());

        // Shift value too large
        let value = json!({
            "shift": 100,
            "upload_bytes": "1MB"
        });
        assert!(as_udp_sock_speed_limit(&value).is_err());

        // Invalid value type
        let value = json!({"shift": "abc"});
        assert!(as_udp_sock_speed_limit(&value).is_err());
    }

    #[test]
    fn as_global_stream_speed_limit_ok() {
        // String input
        let value = json!("1GB");
        let config = as_global_stream_speed_limit(&value).unwrap();
        assert_eq!(config.replenish_bytes(), 1_000_000_000);
        assert_eq!(
            config.replenish_interval(),
            std::time::Duration::from_secs(1)
        );

        // Number input
        let value = json!(102400000);
        let config = as_global_stream_speed_limit(&value).unwrap();
        assert_eq!(config.replenish_bytes(), 102400000);

        // Object input
        let value = json!({
            "replenish_interval": "500ms",
            "replenish_bytes": "1MB",
            "max_burst_bytes": "2MB"
        });
        let config = as_global_stream_speed_limit(&value).unwrap();
        assert_eq!(
            config.replenish_interval(),
            std::time::Duration::from_millis(500)
        );
        assert_eq!(config.replenish_bytes(), 1_000_000);
        assert_eq!(config.max_burst_bytes(), 2_000_000);
    }

    #[test]
    fn as_global_stream_speed_limit_err() {
        // Invalid type (array)
        let value = json!([]);
        assert!(as_global_stream_speed_limit(&value).is_err());

        // Missing replenish_bytes
        let value = json!({"max_burst_bytes": "1GB"});
        assert!(as_global_stream_speed_limit(&value).is_err());

        // Invalid key
        let value = json!({"invalid_key": 100});
        assert!(as_global_stream_speed_limit(&value).is_err());

        // Invalid value type
        let value = json!({"replenish_interval": "abc"});
        assert!(as_global_stream_speed_limit(&value).is_err());
    }

    #[test]
    fn as_global_datagram_speed_limit_ok() {
        // String input
        let value = json!("500KB");
        let config = as_global_datagram_speed_limit(&value).unwrap();
        assert_eq!(config.replenish_bytes(), 500_000);
        assert_eq!(config.replenish_packets(), 0);

        // Number input
        let value = json!(51200);
        let config = as_global_datagram_speed_limit(&value).unwrap();
        assert_eq!(config.replenish_bytes(), 51200);

        // Object input with packets and bytes
        let value = json!({
            "replenish_interval": "1s",
            "replenish_bytes": "500KB",
            "replenish_packets": 100,
            "max_burst_bytes": "1MB",
            "max_burst_packets": 200
        });
        let config = as_global_datagram_speed_limit(&value).unwrap();
        assert_eq!(
            config.replenish_interval(),
            std::time::Duration::from_secs(1)
        );
        assert_eq!(config.replenish_bytes(), 500_000);
        assert_eq!(config.replenish_packets(), 100);
        assert_eq!(config.max_burst_bytes(), 1_000_000);
        assert_eq!(config.max_burst_packets(), 200);
    }

    #[test]
    fn as_global_datagram_speed_limit_err() {
        // Invalid type (boolean)
        let value = json!(true);
        assert!(as_global_datagram_speed_limit(&value).is_err());

        // Missing replenish values
        let value = json!({"replenish_interval": "1s"});
        assert!(as_global_datagram_speed_limit(&value).is_err());

        // Invalid key
        let value = json!({"invalid_key": 100});
        assert!(as_global_datagram_speed_limit(&value).is_err());

        // Invalid value type
        let value = json!({"replenish_bytes": "abc"});
        assert!(as_global_datagram_speed_limit(&value).is_err());
    }
}
