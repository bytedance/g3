/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use serde_json::Value;

use g3_types::net::{TcpConnectConfig, TcpKeepAliveConfig, TcpMiscSockOpts};

pub fn as_tcp_connect_config(v: &Value) -> anyhow::Result<TcpConnectConfig> {
    if let Value::Object(map) = v {
        let mut config = TcpConnectConfig::default();

        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "max_retry" => {
                    let max_retry = crate::value::as_usize(v)
                        .context(format!("invalid usize value for key {k}"))?;
                    config.set_max_retry(max_retry);
                }
                "each_timeout" => {
                    let each_timeout = crate::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    config.set_each_timeout(each_timeout);
                }
                _ => return Err(anyhow!("invalid key {k}")),
            }
        }

        Ok(config)
    } else {
        Err(anyhow!(
            "json value type for 'TcpConnectConfig' should be 'map'"
        ))
    }
}

pub fn as_tcp_keepalive_config(v: &Value) -> anyhow::Result<TcpKeepAliveConfig> {
    let mut config = TcpKeepAliveConfig::default();

    match v {
        Value::Object(map) => {
            for (k, v) in map {
                match crate::key::normalize(k).as_str() {
                    "enable" => {
                        let enable = crate::value::as_bool(v)
                            .context(format!("invalid boolean value for key {k}"))?;
                        config.set_enable(enable);
                    }
                    "idle_time" => {
                        let idle_time = crate::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_idle_time(idle_time);
                    }
                    "probe_interval" => {
                        let interval = crate::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_probe_interval(interval);
                    }
                    "probe_count" => {
                        let count = crate::value::as_u32(v)
                            .context(format!("invalid u32 value for key {k}"))?;
                        config.set_probe_count(count);
                    }
                    _ => return Err(anyhow!("invalid key {k}")),
                }
            }
        }
        Value::Bool(enable) => {
            config.set_enable(*enable);
        }
        _ => {
            let idle_time =
                crate::humanize::as_duration(v).context("invalid tcp keepalive idle_time value")?;
            config.set_enable(true);
            config.set_idle_time(idle_time);
        }
    }

    Ok(config)
}

pub fn as_tcp_misc_sock_opts(v: &Value) -> anyhow::Result<TcpMiscSockOpts> {
    let mut config = TcpMiscSockOpts::default();

    if let Value::Object(map) = v {
        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "no_delay" => {
                    let no_delay = crate::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                    config.no_delay = Some(no_delay);
                }
                "max_segment_size" | "mss" => {
                    let mss = crate::value::as_u32(v)
                        .context(format!("invalid u32 value for key {k}"))?;
                    config.max_segment_size = Some(mss);
                }
                "time_to_live" | "ttl" => {
                    let ttl = crate::value::as_u32(v)
                        .context(format!("invalid u32 value for key {k}"))?;
                    config.time_to_live = Some(ttl);
                }
                "hop_limit" => {
                    let hops = crate::value::as_u32(v)
                        .context(format!("invalid u32 value for key {k}"))?;
                    config.hop_limit = Some(hops);
                }
                "type_of_service" | "tos" => {
                    let tos =
                        crate::value::as_u8(v).context(format!("invalid u8 value for key {k}"))?;
                    config.type_of_service = Some(tos);
                }
                #[cfg(not(windows))]
                "traffic_class" => {
                    let class =
                        crate::value::as_u8(v).context(format!("invalid u8 value for key {k}"))?;
                    config.traffic_class = Some(class);
                }
                #[cfg(any(
                    target_os = "linux",
                    target_os = "freebsd",
                    target_os = "solaris",
                    target_os = "illumos"
                ))]
                "congestion_control" => {
                    let ca = crate::value::as_string(v)?;
                    config.set_congestion_control(ca);
                }
                #[cfg(target_os = "linux")]
                "netfilter_mark" | "mark" => {
                    let mark = crate::value::as_u32(v)
                        .context(format!("invalid u32 value for key {k}"))?;
                    config.netfilter_mark = Some(mark);
                }
                _ => return Err(anyhow!("invalid key {k}")),
            }
        }

        Ok(config)
    } else {
        Err(anyhow!(
            "json value type for 'TcpMiscSockOpts' should be 'map'"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn as_tcp_connect_config_ok() {
        // valid configuration
        let valid = json!({
            "max_retry": 5,
            "each_timeout": "30s"
        });
        let config = as_tcp_connect_config(&valid).unwrap();
        assert_eq!(config.max_tries(), 6);
        assert_eq!(config.each_timeout(), Duration::from_secs(30));
    }

    #[test]
    fn as_tcp_connect_config_err() {
        // invalid key
        let invalid_key = json!({"invalid_key": 10});
        assert!(as_tcp_connect_config(&invalid_key).is_err());

        // invalid value type
        let invalid_value = json!({"max_retry": "string"});
        assert!(as_tcp_connect_config(&invalid_value).is_err());

        // non-object input
        let non_object = json!(["array"]);
        assert!(as_tcp_connect_config(&non_object).is_err());
    }

    #[test]
    fn as_tcp_keepalive_config_ok() {
        // object form with all keys
        let obj = json!({
            "enable": true,
            "idle_time": "60s",
            "probe_interval": "5s",
            "probe_count": 3
        });
        let config = as_tcp_keepalive_config(&obj).unwrap();
        assert!(config.is_enabled());
        assert_eq!(config.idle_time(), Duration::from_secs(60));
        assert_eq!(config.probe_interval(), Some(Duration::from_secs(5)));
        assert_eq!(config.probe_count(), Some(3));

        // boolean form
        let bool_true = json!(true);
        let config = as_tcp_keepalive_config(&bool_true).unwrap();
        assert!(config.is_enabled());

        let bool_false = json!(false);
        let config = as_tcp_keepalive_config(&bool_false).unwrap();
        assert!(!config.is_enabled());

        // duration string form
        let duration_str = json!("30s");
        let config = as_tcp_keepalive_config(&duration_str).unwrap();
        assert!(config.is_enabled());
        assert_eq!(config.idle_time(), Duration::from_secs(30));
    }

    #[test]
    fn as_tcp_keepalive_config_err() {
        // invalid key
        let invalid_key = json!({"invalid_key": true});
        assert!(as_tcp_keepalive_config(&invalid_key).is_err());

        // invalid value type
        let invalid_value = json!({"idle_time": true});
        assert!(as_tcp_keepalive_config(&invalid_value).is_err());

        // invalid duration format
        let invalid_duration = json!("invalid");
        assert!(as_tcp_keepalive_config(&invalid_duration).is_err());
    }

    #[test]
    fn as_tcp_misc_sock_opts_ok() {
        // common keys supported on all platforms
        let common_json = json!({
            "no_delay": true,
            "max_segment_size": 1460,
            "ttl": 64,
            "hop_limit": 64,
            "tos": 32,
        });

        let config = as_tcp_misc_sock_opts(&common_json).unwrap();
        assert_eq!(config.no_delay, Some(true));
        assert_eq!(config.max_segment_size, Some(1460));
        assert_eq!(config.time_to_live, Some(64));
        assert_eq!(config.hop_limit, Some(64));
        assert_eq!(config.type_of_service, Some(32));

        // traffic_class on non-Windows platforms
        #[cfg(not(windows))]
        {
            let traffic_json = json!({"traffic_class": 16});
            let config = as_tcp_misc_sock_opts(&traffic_json).unwrap();
            assert_eq!(config.traffic_class, Some(16));
        }

        // platform-specific keys
        #[cfg(any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "solaris",
            target_os = "illumos"
        ))]
        {
            let congestion_json = json!({"congestion_control": "cubic"});
            let config = as_tcp_misc_sock_opts(&congestion_json).unwrap();
            assert_eq!(config.congestion_control().unwrap(), b"cubic");
        }

        #[cfg(target_os = "linux")]
        {
            let mark_json = json!({"mark": 12345});
            let config = as_tcp_misc_sock_opts(&mark_json).unwrap();
            assert_eq!(config.netfilter_mark, Some(12345));
        }
    }

    #[test]
    fn as_tcp_misc_sock_opts_err() {
        // invalid key
        let invalid_key = json!({"invalid_key": true});
        assert!(as_tcp_misc_sock_opts(&invalid_key).is_err());

        // invalid value type
        let invalid_value = json!({"no_delay": "string"});
        assert!(as_tcp_misc_sock_opts(&invalid_value).is_err());

        // non-object input
        let non_object = json!(123);
        assert!(as_tcp_misc_sock_opts(&non_object).is_err());
    }
}
