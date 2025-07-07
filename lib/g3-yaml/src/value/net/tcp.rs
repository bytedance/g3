/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::net::{
    HappyEyeballsConfig, TcpConnectConfig, TcpKeepAliveConfig, TcpListenConfig, TcpMiscSockOpts,
};

fn set_tcp_listen_scale(config: &mut TcpListenConfig, v: &Yaml) -> anyhow::Result<()> {
    match v {
        Yaml::String(s) => {
            if s.ends_with('%') {
                let Ok(v) = f64::from_str(&s[..s.len() - 1]) else {
                    return Err(anyhow!("invalid percentage value {s}"));
                };
                config
                    .set_scale(v / 100.0)
                    .context(format!("unsupported percentage value {s}"))
            } else if let Some((n, d)) = s.split_once('/') {
                let Ok(n) = usize::from_str(n.trim()) else {
                    return Err(anyhow!("invalid fractional value {s}: invalid numerator"))?;
                };
                let Ok(d) = usize::from_str(d.trim()) else {
                    return Err(anyhow!("invalid fractional value {s}: invalid denominator"))?;
                };
                config.set_fraction_scale(n, d);
                Ok(())
            } else {
                let Ok(v) = f64::from_str(s) else {
                    return Err(anyhow!("invalid float value: {s}"));
                };
                config
                    .set_scale(v)
                    .context(format!("unsupported float value {s}"))
            }
        }
        Yaml::Integer(i) => config
            .set_scale(*i as f64)
            .context(format!("unsupported integer value {i}")),
        Yaml::Real(s) => {
            let Ok(v) = f64::from_str(s) else {
                return Err(anyhow!("invalid float value: {s}"));
            };
            config
                .set_scale(v)
                .context(format!("unsupported float value {s}"))
        }
        _ => Err(anyhow!(
            "yaml value type for tcp listen scale value should be 'str' or 'float'"
        )),
    }
}

pub fn as_tcp_listen_config(value: &Yaml) -> anyhow::Result<TcpListenConfig> {
    let mut config = TcpListenConfig::default();

    match value {
        Yaml::Integer(i) => {
            let port = u16::try_from(*i).map_err(|e| anyhow!("out of range u16 value: {e}"))?;
            config.set_port(port);
        }
        Yaml::String(_) => {
            let addr = crate::value::as_env_sockaddr(value)
                .context("invalid tcp listen socket address value")?;
            config.set_socket_address(addr);
        }
        Yaml::Hash(map) => {
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "addr" | "address" => {
                    let addr = crate::value::as_env_sockaddr(v).context(format!(
                        "invalid tcp listen socket address value for key {k}"
                    ))?;
                    config.set_socket_address(addr);
                    Ok(())
                }
                #[cfg(any(
                    target_os = "linux",
                    target_os = "android",
                    target_os = "macos",
                    target_os = "illumos",
                    target_os = "solaris"
                ))]
                "interface" => {
                    let interface = crate::value::as_interface(v)
                        .context(format!("invalid interface name value for key {k}"))?;
                    config.set_interface(interface);
                    Ok(())
                }
                "backlog" => {
                    let backlog = crate::value::as_u32(v)
                        .context(format!("invalid u32 value for key {k}"))?;
                    config.set_backlog(backlog);
                    Ok(())
                }
                #[cfg(not(target_os = "openbsd"))]
                "ipv6only" | "ipv6_only" => {
                    let ipv6only = crate::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                    config.set_ipv6_only(ipv6only);
                    Ok(())
                }
                "instance" | "instance_count" => {
                    let instance = crate::value::as_usize(v)
                        .context(format!("invalid usize value for key {k}"))?;
                    config.set_instance(instance);
                    Ok(())
                }
                #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
                "netfilter_mark" | "fwmark" | "mark" => {
                    let mark = crate::value::as_u32(v)
                        .context(format!("invalid u32 value for key {k}"))?;
                    config.set_mark(mark);
                    Ok(())
                }
                "scale" => set_tcp_listen_scale(&mut config, v)
                    .context(format!("invalid scale value for key {k}")),
                "follow_cpu_affinity" => {
                    let enable = crate::value::as_bool(v)?;
                    config.set_follow_cpu_affinity(enable);
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
        }
        _ => return Err(anyhow!("invalid value type")),
    }

    config.check()?;
    Ok(config)
}

pub fn as_tcp_connect_config(v: &Yaml) -> anyhow::Result<TcpConnectConfig> {
    if let Yaml::Hash(map) = v {
        let mut config = TcpConnectConfig::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "max_retry" => {
                let max_retry = crate::value::as_usize(v)?;
                config.set_max_retry(max_retry);
                Ok(())
            }
            "each_timeout" => {
                let each_timeout = crate::humanize::as_duration(v)?;
                config.set_each_timeout(each_timeout);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'TcpConnectConfig' should be 'map'"
        ))
    }
}

pub fn as_happy_eyeballs_config(v: &Yaml) -> anyhow::Result<HappyEyeballsConfig> {
    if let Yaml::Hash(map) = v {
        let mut config = HappyEyeballsConfig::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "resolution_delay" | "first_resolution_delay" => {
                let delay = crate::humanize::as_duration(v)?;
                config.set_resolution_delay(delay);
                Ok(())
            }
            "second_resolution_timeout" => {
                let timeout = crate::humanize::as_duration(v)?;
                config.set_second_resolution_timeout(timeout);
                Ok(())
            }
            "first_address_family_count" => {
                let count = crate::value::as_usize(v)?;
                config.set_first_address_family_count(count);
                Ok(())
            }
            "connection_attempt_delay" => {
                let delay = crate::humanize::as_duration(v)?;
                config.set_connection_attempt_delay(delay);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'HappyEyeballsConfig' should be 'map'"
        ))
    }
}

pub fn as_tcp_keepalive_config(v: &Yaml) -> anyhow::Result<TcpKeepAliveConfig> {
    let mut config = TcpKeepAliveConfig::default();

    match v {
        Yaml::Hash(map) => {
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "enable" => {
                    let enable = crate::value::as_bool(v)?;
                    config.set_enable(enable);
                    Ok(())
                }
                "idle_time" => {
                    let idle_time = crate::humanize::as_duration(v)?;
                    config.set_idle_time(idle_time);
                    Ok(())
                }
                "probe_interval" => {
                    let interval = crate::humanize::as_duration(v)?;
                    config.set_probe_interval(interval);
                    Ok(())
                }
                "probe_count" => {
                    let count = crate::value::as_u32(v)?;
                    config.set_probe_count(count);
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
        }
        Yaml::Boolean(enable) => {
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

pub fn as_tcp_misc_sock_opts(v: &Yaml) -> anyhow::Result<TcpMiscSockOpts> {
    let mut config = TcpMiscSockOpts::default();

    if let Yaml::Hash(map) = v {
        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "no_delay" => {
                let no_delay =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                config.no_delay = Some(no_delay);
                Ok(())
            }
            "max_segment_size" | "mss" => {
                let mss =
                    crate::value::as_u32(v).context(format!("invalid u32 value for key {k}"))?;
                config.max_segment_size = Some(mss);
                Ok(())
            }
            "time_to_live" | "ttl" => {
                let ttl =
                    crate::value::as_u32(v).context(format!("invalid u32 value for key {k}"))?;
                config.time_to_live = Some(ttl);
                Ok(())
            }
            "hop_limit" => {
                let hops =
                    crate::value::as_u32(v).context(format!("invalid u32 value for key {k}"))?;
                config.hop_limit = Some(hops);
                Ok(())
            }
            "type_of_service" | "tos" => {
                let tos =
                    crate::value::as_u8(v).context(format!("invalid u8 value for key {k}"))?;
                config.type_of_service = Some(tos);
                Ok(())
            }
            #[cfg(not(windows))]
            "traffic_class" => {
                let class =
                    crate::value::as_u8(v).context(format!("invalid u8 value for key {k}"))?;
                config.traffic_class = Some(class);
                Ok(())
            }
            #[cfg(target_os = "linux")]
            "netfilter_mark" | "mark" => {
                let mark =
                    crate::value::as_u32(v).context(format!("invalid u32 value for key {k}"))?;
                config.netfilter_mark = Some(mark);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'TcpMiscSockOpts' should be 'map'"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv6Addr, SocketAddr};
    use std::time::Duration;
    use yaml_rust::{Yaml, YamlLoader};

    use g3_types::net::{
        HappyEyeballsConfig, TcpConnectConfig, TcpKeepAliveConfig, TcpListenConfig, TcpMiscSockOpts,
    };

    #[test]
    fn as_tcp_listen_config_ok() {
        let yaml = yaml_doc!("8080");
        let config = as_tcp_listen_config(&yaml).unwrap();
        assert_eq!(config.address().port(), 8080);
        assert_eq!(
            config.address(),
            SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 8080)
        );

        let yaml = yaml_doc!("\"127.0.0.1:8081\"");
        let config = as_tcp_listen_config(&yaml).unwrap();
        let expected_addr: SocketAddr = "127.0.0.1:8081".parse().unwrap();
        assert_eq!(config.address(), expected_addr);

        let yaml = yaml_doc!(
            r#"
                address: "0.0.0.0:8083"
                backlog: 1024
                scale: "50%"
            "#
        );
        let config = as_tcp_listen_config(&yaml).unwrap();
        let expected_addr: SocketAddr = "0.0.0.0:8083".parse().unwrap();
        assert_eq!(config.address(), expected_addr);
        assert_eq!(config.backlog(), 1024);

        #[cfg(not(target_os = "openbsd"))]
        let yaml = yaml_doc!(
            r#"
                address: "[::]:8083"
                ipv6_only: true
            "#
        );
        let config = as_tcp_listen_config(&yaml).unwrap();
        assert_eq!(config.is_ipv6only(), Some(true));

        #[cfg(not(target_os = "openbsd"))]
        let yaml = yaml_doc!(
            r#"
                address: "[::]:8084"
                ipv6_only: false
            "#
        );
        let config = as_tcp_listen_config(&yaml).unwrap();
        assert_eq!(config.is_ipv6only(), Some(false));

        let yaml_map = yaml_doc!("scale: \"50%\"");
        let mut cfg = TcpListenConfig::default();
        assert!(set_tcp_listen_scale(&mut cfg, &yaml_map["scale"]).is_ok());

        let yaml_map = yaml_doc!("scale: \"3/4\"");
        let mut cfg = TcpListenConfig::default();
        assert!(set_tcp_listen_scale(&mut cfg, &yaml_map["scale"]).is_ok());

        let yaml_map = yaml_doc!("scale: \"1.5\"");
        let mut cfg = TcpListenConfig::default();
        assert!(set_tcp_listen_scale(&mut cfg, &yaml_map["scale"]).is_ok());

        let yaml_map = yaml_doc!("scale: 2");
        let mut cfg = TcpListenConfig::default();
        assert!(set_tcp_listen_scale(&mut cfg, &yaml_map["scale"]).is_ok());

        let yaml_value = Yaml::Real("2.5".to_string());
        let mut cfg = TcpListenConfig::default();
        assert!(set_tcp_listen_scale(&mut cfg, &yaml_value).is_ok());
    }

    #[test]
    fn as_tcp_listen_config_err() {
        let yaml = yaml_doc!("70000");
        assert!(as_tcp_listen_config(&yaml).is_err());

        let yaml = yaml_doc!("\"not_an_address\"");
        assert!(as_tcp_listen_config(&yaml).is_err());

        let yaml_map = yaml_doc!("scale: true");
        let mut cfg = TcpListenConfig::default();
        assert!(set_tcp_listen_scale(&mut cfg, &yaml_map["scale"]).is_err());

        let yaml_map = yaml_doc!("scale: \"abc%\"");
        let mut cfg = TcpListenConfig::default();
        assert!(set_tcp_listen_scale(&mut cfg, &yaml_map["scale"]).is_err());

        let yaml_map = yaml_doc!("scale: \"a/4\"");
        let mut cfg = TcpListenConfig::default();
        assert!(set_tcp_listen_scale(&mut cfg, &yaml_map["scale"]).is_err());

        let yaml_map = yaml_doc!("scale: \"3/b\"");
        let mut cfg = TcpListenConfig::default();
        assert!(set_tcp_listen_scale(&mut cfg, &yaml_map["scale"]).is_err());

        let yaml_map = yaml_doc!("scale: \"not_a_float\"");
        let mut cfg = TcpListenConfig::default();
        assert!(set_tcp_listen_scale(&mut cfg, &yaml_map["scale"]).is_err());

        let yaml = yaml_doc!("invalid_key: 123");
        assert!(as_tcp_listen_config(&yaml).is_err());

        let yaml = yaml_doc!("[1, 2, 3]");
        assert!(as_tcp_listen_config(&yaml).is_err());
    }

    #[test]
    fn as_tcp_connect_config_ok() {
        let yaml = yaml_doc!(
            r#"
                max_retry: 5
                each_timeout: 10s
            "#
        );
        let config = as_tcp_connect_config(&yaml).unwrap();
        assert_eq!(config.max_tries(), 6);
        assert_eq!(config.each_timeout(), Duration::from_secs(10));

        let yaml = yaml_doc!("{}");
        let config = as_tcp_connect_config(&yaml).unwrap();
        let default_config = TcpConnectConfig::default();
        assert_eq!(config.max_tries(), default_config.max_tries());
        assert_eq!(config.each_timeout(), default_config.each_timeout());
    }

    #[test]
    fn as_tcp_connect_config_err() {
        let yaml = yaml_doc!("123");
        assert!(as_tcp_connect_config(&yaml).is_err());

        let yaml = yaml_doc!("unknown_key: 100");
        assert!(as_tcp_connect_config(&yaml).is_err());

        let yaml = yaml_doc!("max_retry: \"not_a_number\"");
        assert!(as_tcp_connect_config(&yaml).is_err());

        let yaml = yaml_doc!("each_timeout: \"not_a_duration\"");
        assert!(as_tcp_connect_config(&yaml).is_err());
    }

    #[test]
    fn as_happy_eyeballs_config_ok() {
        let yaml = yaml_doc!(
            r#"
                resolution_delay: 50ms
                second_resolution_timeout: 1s
                first_address_family_count: 2
                connection_attempt_delay: 25ms
            "#
        );
        let config = as_happy_eyeballs_config(&yaml).unwrap();
        assert_eq!(config.resolution_delay(), Duration::from_millis(50));
        assert_eq!(config.second_resolution_timeout(), Duration::from_secs(1));
        assert_eq!(config.first_address_family_count(), 2);
        assert_eq!(
            config.connection_attempt_delay(),
            Duration::from_millis(100)
        );

        let yaml = yaml_doc!("{}");
        let config = as_happy_eyeballs_config(&yaml).unwrap();
        let default_config = HappyEyeballsConfig::default();
        assert_eq!(config.resolution_delay(), default_config.resolution_delay());
        assert_eq!(
            config.second_resolution_timeout(),
            default_config.second_resolution_timeout()
        );
        assert_eq!(
            config.first_address_family_count(),
            default_config.first_address_family_count()
        );
        assert_eq!(
            config.connection_attempt_delay(),
            default_config.connection_attempt_delay()
        );
    }

    #[test]
    fn as_happy_eyeballs_config_err() {
        let yaml = yaml_doc!("\"string_value\"");
        assert!(as_happy_eyeballs_config(&yaml).is_err());

        let yaml = yaml_doc!("bad_key: true");
        assert!(as_happy_eyeballs_config(&yaml).is_err());

        let yaml = yaml_doc!("resolution_delay: \"-1s\"");
        assert!(as_happy_eyeballs_config(&yaml).is_err());
    }

    #[test]
    fn as_tcp_keepalive_config_ok() {
        let yaml = yaml_doc!(
            r#"
                enable: true
                idle_time: 300s
                probe_interval: 10s
                probe_count: 5
            "#
        );
        let config = as_tcp_keepalive_config(&yaml).unwrap();
        assert!(config.is_enabled());
        assert_eq!(config.idle_time(), Duration::from_secs(300));
        assert_eq!(config.probe_interval(), Some(Duration::from_secs(10)));
        assert_eq!(config.probe_count(), Some(5));

        let yaml = yaml_doc!("true");
        let config = as_tcp_keepalive_config(&yaml).unwrap();
        assert!(config.is_enabled());
        let default_tcp_ka_config = TcpKeepAliveConfig::default();
        assert_eq!(config.idle_time(), default_tcp_ka_config.idle_time());

        let yaml = yaml_doc!("false");
        let config = as_tcp_keepalive_config(&yaml).unwrap();
        assert!(!config.is_enabled());

        let yaml = yaml_doc!("\"120s\"");
        let config = as_tcp_keepalive_config(&yaml).unwrap();
        assert!(config.is_enabled());
        assert_eq!(config.idle_time(), Duration::from_secs(120));

        let yaml = yaml_doc!("enable: false");
        let config = as_tcp_keepalive_config(&yaml).unwrap();
        assert!(!config.is_enabled());
    }

    #[test]
    fn as_tcp_keepalive_config_err() {
        let yaml = yaml_doc!("unknown_field: 10s");
        assert!(as_tcp_keepalive_config(&yaml).is_err());

        let yaml = yaml_doc!("idle_time: -1s");
        assert!(as_tcp_keepalive_config(&yaml).is_err());

        let yaml = yaml_doc!("probe_interval: -1s");
        assert!(as_tcp_keepalive_config(&yaml).is_err());

        let yaml = yaml_doc!("probe_count: -1");
        assert!(as_tcp_keepalive_config(&yaml).is_err());

        let yaml = yaml_doc!("enable: \"not_a_bool\"");
        assert!(as_tcp_keepalive_config(&yaml).is_err());
    }

    #[test]
    fn as_tcp_misc_sock_opts_ok() {
        let yaml = yaml_doc!(
            r#"
                no_delay: true
                max_segment_size: 1460
                time_to_live: 64
                hop_limit: 64
                type_of_service: 0x10
            "#
        );
        let config = as_tcp_misc_sock_opts(&yaml).unwrap();
        assert_eq!(config.no_delay, Some(true));
        assert_eq!(config.max_segment_size, Some(1460));
        assert_eq!(config.time_to_live, Some(64));
        assert_eq!(config.hop_limit, Some(64));
        assert_eq!(config.type_of_service, Some(0x10));

        let yaml = yaml_doc!("{}");
        let config = as_tcp_misc_sock_opts(&yaml).unwrap();
        let default_config = TcpMiscSockOpts::default();
        assert_eq!(config.no_delay, default_config.no_delay);
        assert_eq!(config.max_segment_size, default_config.max_segment_size);
        assert_eq!(config.time_to_live, default_config.time_to_live);
        assert_eq!(config.type_of_service, default_config.type_of_service);
        #[cfg(target_os = "linux")]
        assert_eq!(config.netfilter_mark, default_config.netfilter_mark);
    }

    #[test]
    fn as_tcp_misc_sock_opts_err() {
        let yaml = yaml_doc!("\"some_string\"");
        assert!(as_tcp_misc_sock_opts(&yaml).is_err());

        let yaml = yaml_doc!("unsupported_opt: 1");
        assert!(as_tcp_misc_sock_opts(&yaml).is_err());

        let yaml = yaml_doc!("no_delay: \"true_string\"");
        assert!(as_tcp_misc_sock_opts(&yaml).is_err());

        let yaml = yaml_doc!("max_segment_size: \"1460s\"");
        assert!(as_tcp_misc_sock_opts(&yaml).is_err());

        let yaml = yaml_doc!("type_of_service: \"not_u8\"");
        assert!(as_tcp_misc_sock_opts(&yaml).is_err());
    }
}
