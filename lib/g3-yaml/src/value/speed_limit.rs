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

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    #[test]
    fn as_tcp_sock_speed_limit_ok() {
        // string input
        let yaml = yaml_str!("10MB");
        let config = as_tcp_sock_speed_limit(&yaml).unwrap();
        assert_eq!(config.max_north, 10_000_000);
        assert_eq!(config.max_south, 10_000_000);
        assert_eq!(
            config.shift_millis,
            g3_types::net::RATE_LIMIT_SHIFT_MILLIS_DEFAULT
        );

        // integer input
        let yaml = Yaml::Integer(102400);
        let config = as_tcp_sock_speed_limit(&yaml).unwrap();
        assert_eq!(config.max_north, 102400);
        assert_eq!(config.max_south, 102400);
        assert_eq!(
            config.shift_millis,
            g3_types::net::RATE_LIMIT_SHIFT_MILLIS_DEFAULT
        );

        // hash input
        let yaml = yaml_doc!(
            r#"
                shift: 5
                upload: 5MB
                download: 10MB
            "#
        );
        let config = as_tcp_sock_speed_limit(&yaml).unwrap();
        assert_eq!(config.shift_millis, 5);
        assert_eq!(config.max_north, 5_000_000);
        assert_eq!(config.max_south, 10_000_000);

        let yaml = yaml_doc!(
            r#"
                shift_millis: 10
                upload_bytes: 1MB
                download_bytes: 2MB
            "#
        );
        let config = as_tcp_sock_speed_limit(&yaml).unwrap();
        assert_eq!(config.shift_millis, 10);
        assert_eq!(config.max_north, 1_000_000);
        assert_eq!(config.max_south, 2_000_000);

        let yaml = yaml_doc!(
            r#"
                north: 10MB
                south: 20MB
            "#
        );
        let config = as_tcp_sock_speed_limit(&yaml).unwrap();
        assert_eq!(config.shift_millis, 0);
        assert_eq!(config.max_north, 10_000_000);
        assert_eq!(config.max_south, 20_000_000);

        let yaml = yaml_doc!(
            r#"
                north_bytes: 1MB
                south_bytes: 2MB
            "#
        );
        let config = as_tcp_sock_speed_limit(&yaml).unwrap();
        assert_eq!(config.shift_millis, 0);
        assert_eq!(config.max_north, 1_000_000);
        assert_eq!(config.max_south, 2_000_000);
    }

    #[test]
    fn as_tcp_sock_speed_limit_err() {
        // invalid type
        let yaml = Yaml::Array(vec![]);
        assert!(as_tcp_sock_speed_limit(&yaml).is_err());

        // invalid key
        let yaml = yaml_doc!(
            r#"
                invalid_key: 100
            "#
        );
        assert!(as_tcp_sock_speed_limit(&yaml).is_err());

        // shift value too large
        let yaml = yaml_doc!(
            r#"
                shift: 100
                upload: 1MB
            "#
        );
        assert!(as_tcp_sock_speed_limit(&yaml).is_err());

        // upload limit zero when shift is set
        let yaml = yaml_doc!(
            r#"
                shift: 5
                upload: 0
            "#
        );
        assert!(as_tcp_sock_speed_limit(&yaml).is_err());

        // invalid value
        let yaml = yaml_doc!(
            r#"
                shift: abc
            "#
        );
        assert!(as_tcp_sock_speed_limit(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                north: abc
            "#
        );
        assert!(as_tcp_sock_speed_limit(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                south: abc
            "#
        );
        assert!(as_tcp_sock_speed_limit(&yaml).is_err());

        let yaml = yaml_str!("abc");
        assert!(as_tcp_sock_speed_limit(&yaml).is_err());
    }

    #[test]
    fn as_udp_sock_speed_limit_ok() {
        // string input
        let yaml = yaml_str!("5MB");
        let config = as_udp_sock_speed_limit(&yaml).unwrap();
        assert_eq!(config.max_north_bytes, 5_000_000);
        assert_eq!(config.max_south_bytes, 5_000_000);
        assert_eq!(
            config.shift_millis,
            g3_types::net::RATE_LIMIT_SHIFT_MILLIS_DEFAULT
        );

        // integer input
        let yaml = Yaml::Integer(51200);
        let config = as_udp_sock_speed_limit(&yaml).unwrap();
        assert_eq!(config.max_north_bytes, 51200);
        assert_eq!(config.max_south_bytes, 51200);
        assert_eq!(
            config.shift_millis,
            g3_types::net::RATE_LIMIT_SHIFT_MILLIS_DEFAULT
        );

        // hash input
        let yaml = yaml_doc!(
            r#"
                shift: 4
                upload_packets: 500
                download_packets: 1000
                upload_bytes: 2MB
                download_bytes: 4MB
            "#
        );
        let config = as_udp_sock_speed_limit(&yaml).unwrap();
        assert_eq!(config.shift_millis, 4);
        assert_eq!(config.max_north_packets, 500);
        assert_eq!(config.max_south_packets, 1000);
        assert_eq!(config.max_north_bytes, 2_000_000);
        assert_eq!(config.max_south_bytes, 4_000_000);

        let yaml = yaml_doc!(
            r#"
                shift_millis: 10
                north_packets: 1000
                south_packets: 2000
                north_bytes: 1MB
                south_bytes: 2MB
            "#
        );
        let config = as_udp_sock_speed_limit(&yaml).unwrap();
        assert_eq!(config.shift_millis, 10);
        assert_eq!(config.max_north_packets, 1000);
        assert_eq!(config.max_south_packets, 2000);
        assert_eq!(config.max_north_bytes, 1_000_000);
        assert_eq!(config.max_south_bytes, 2_000_000);
    }

    #[test]
    fn as_udp_sock_speed_limit_err() {
        // invalid type
        let yaml = Yaml::Null;
        assert!(as_udp_sock_speed_limit(&yaml).is_err());

        // invalid key
        let yaml = yaml_doc!(
            r#"
                invalid_key: 100
            "#
        );
        assert!(as_udp_sock_speed_limit(&yaml).is_err());

        // shift value too large
        let yaml = yaml_doc!(
            r#"
                shift: 100
                upload_bytes: 1MB
            "#
        );
        assert!(as_udp_sock_speed_limit(&yaml).is_err());

        // invalid value
        let yaml = yaml_doc!(
            r#"
                shift: abc
            "#
        );
        assert!(as_udp_sock_speed_limit(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                north_bytes: def
            "#
        );
        assert!(as_udp_sock_speed_limit(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                south_bytes: ghi
            "#
        );
        assert!(as_udp_sock_speed_limit(&yaml).is_err());

        let yaml = yaml_str!("abc");
        assert!(as_udp_sock_speed_limit(&yaml).is_err());
    }

    #[test]
    fn as_global_stream_speed_limit_ok() {
        // string input
        let yaml = yaml_str!("1GB");
        let config = as_global_stream_speed_limit(&yaml).unwrap();
        assert_eq!(config.replenish_bytes(), 1_000_000_000);
        assert_eq!(
            config.replenish_interval(),
            std::time::Duration::from_secs(1)
        );

        // integer input
        let yaml = Yaml::Integer(102400000);
        let config = as_global_stream_speed_limit(&yaml).unwrap();
        assert_eq!(config.replenish_bytes(), 102400000);
        assert_eq!(
            config.replenish_interval(),
            std::time::Duration::from_secs(1)
        );

        // hash input
        let yaml = yaml_doc!(
            r#"
                replenish_interval: 500ms
                replenish_bytes: 1MB
                max_burst_bytes: 2MB
            "#
        );
        let config = as_global_stream_speed_limit(&yaml).unwrap();
        assert_eq!(
            config.replenish_interval(),
            std::time::Duration::from_millis(500)
        );
        assert_eq!(config.replenish_bytes(), 1_000_000);
        assert_eq!(config.max_burst_bytes(), 2_000_000);
    }

    #[test]
    fn as_global_stream_speed_limit_err() {
        // invalid type
        let yaml = Yaml::Array(vec![]);
        assert!(as_global_stream_speed_limit(&yaml).is_err());

        // no replenish_bytes set
        let yaml = yaml_doc!(
            r#"
                max_burst_bytes: 1GB
            "#
        );
        assert!(as_global_stream_speed_limit(&yaml).is_err());

        // invalid key
        let yaml = yaml_doc!(
            r#"
                invalid_key: 100
            "#
        );
        assert!(as_global_stream_speed_limit(&yaml).is_err());

        // invalid value
        let yaml = yaml_doc!(
            r#"
                replenish_interval: abc
            "#
        );
        assert!(as_global_stream_speed_limit(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                replenish_bytes: def
            "#
        );
        assert!(as_global_stream_speed_limit(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                max_burst_bytes: ghi
            "#
        );
        assert!(as_global_stream_speed_limit(&yaml).is_err());

        let yaml = yaml_str!("abc");
        assert!(as_global_stream_speed_limit(&yaml).is_err());
    }

    #[test]
    fn as_global_datagram_speed_limit_ok() {
        // string input
        let yaml = yaml_str!("500KB");
        let config = as_global_datagram_speed_limit(&yaml).unwrap();
        assert_eq!(config.replenish_bytes(), 500_000);
        assert_eq!(config.replenish_packets(), 0);

        // integer input
        let yaml = Yaml::Integer(51200);
        let config = as_global_datagram_speed_limit(&yaml).unwrap();
        assert_eq!(config.replenish_bytes(), 51200);
        assert_eq!(config.replenish_packets(), 0);

        // hash input
        let yaml = yaml_doc!(
            r#"
                replenish_interval: 1s
                replenish_bytes: 500KB
                replenish_packets: 100
                max_burst_bytes: 1MB
                max_burst_packets: 200
            "#
        );
        let config = as_global_datagram_speed_limit(&yaml).unwrap();
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
        // invalid type
        let yaml = Yaml::Null;
        assert!(as_global_datagram_speed_limit(&yaml).is_err());

        // invalid key
        let yaml = yaml_doc!(
            r#"
                invalid_key: 100
            "#
        );
        assert!(as_global_datagram_speed_limit(&yaml).is_err());

        // no replenish_bytes/packets set
        let yaml = yaml_doc!(
            r#"
                replenish_interval: 1s
            "#
        );
        assert!(as_global_datagram_speed_limit(&yaml).is_err());

        // invalid value
        let yaml = yaml_doc!(
            r#"
                replenish_bytes: abc
            "#
        );
        assert!(as_global_datagram_speed_limit(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                replenish_packets: def
            "#
        );
        assert!(as_global_datagram_speed_limit(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                max_burst_bytes: ghi
            "#
        );
        assert!(as_global_datagram_speed_limit(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                max_burst_packets: jkl
            "#
        );
        assert!(as_global_datagram_speed_limit(&yaml).is_err());

        let yaml = yaml_str!("abc");
        assert!(as_global_datagram_speed_limit(&yaml).is_err());
    }
}
