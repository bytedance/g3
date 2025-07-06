/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::net::{UdpListenConfig, UdpMiscSockOpts};

pub fn as_udp_misc_sock_opts(v: &Yaml) -> anyhow::Result<UdpMiscSockOpts> {
    let mut config = UdpMiscSockOpts::default();

    if let Yaml::Hash(map) = v {
        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
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
            "traffic_class" => {
                let class =
                    crate::value::as_u8(v).context(format!("invalid u8 value for key {k}"))?;
                config.traffic_class = Some(class);
                Ok(())
            }
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
            "yaml value type for 'UdpMiscSockOpts' should be 'map'"
        ))
    }
}

fn set_udp_listen_scale(config: &mut UdpListenConfig, v: &Yaml) -> anyhow::Result<()> {
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
            "yaml value type for udp listen scale value should be 'str' or 'float'"
        )),
    }
}

pub fn as_udp_listen_config(value: &Yaml) -> anyhow::Result<UdpListenConfig> {
    let mut config = UdpListenConfig::default();

    match value {
        Yaml::Integer(i) => {
            let port = u16::try_from(*i).map_err(|e| anyhow!("out of range u16 value: {e}"))?;
            config.set_port(port);
        }
        Yaml::String(_) => {
            let addr = crate::value::as_env_sockaddr(value)
                .context("invalid udp listen socket address value")?;
            config.set_socket_address(addr);
        }
        Yaml::Hash(map) => {
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "addr" | "address" => {
                    let addr = crate::value::as_env_sockaddr(v).context(format!(
                        "invalid udp listen socket address value for key {k}"
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
                #[cfg(not(target_os = "openbsd"))]
                "ipv6only" | "ipv6_only" => {
                    let ipv6only = crate::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                    config.set_ipv6_only(ipv6only);
                    Ok(())
                }
                "socket_buffer" => {
                    let buf_conf = crate::value::as_socket_buffer_config(v)
                        .context(format!("invalid socket buffer config value for key {k}"))?;
                    config.set_socket_buffer(buf_conf);
                    Ok(())
                }
                "socket_misc_opts" => {
                    let misc_opts = as_udp_misc_sock_opts(v)
                        .context(format!("invalid udp socket misc opts value for key {k}"))?;
                    config.set_socket_misc_opts(misc_opts);
                    Ok(())
                }
                "instance" | "instance_count" => {
                    let instance = crate::value::as_usize(v)
                        .context(format!("invalid usize value for key {k}"))?;
                    config.set_instance(instance);
                    Ok(())
                }
                "scale" => set_udp_listen_scale(&mut config, v)
                    .context(format!("invalid scale value for key {k}")),
                _ => Err(anyhow!("invalid key {k}")),
            })?;
        }
        _ => return Err(anyhow!("invalid value type")),
    }

    config.check()?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    #[test]
    fn as_udp_misc_sock_opts_ok() {
        let yaml = yaml_doc!(
            r#"
                time_to_live: 128
                hop_limit: 128
                type_of_service: 0x10
                traffic_class: 0x10
                netfilter_mark: 100
            "#
        );
        let parsed_config = as_udp_misc_sock_opts(&yaml).unwrap();
        let mut expected_config = UdpMiscSockOpts::default();
        expected_config.time_to_live = Some(128);
        expected_config.hop_limit = Some(128);
        expected_config.type_of_service = Some(0x10);
        expected_config.traffic_class = Some(0x10);
        expected_config.netfilter_mark = Some(100);
        assert_eq!(parsed_config, expected_config);

        let yaml = yaml_doc!(
            r#"
                ttl: 64
                tos: 20
                mark: 200
            "#
        );
        let parsed_config = as_udp_misc_sock_opts(&yaml).unwrap();
        let mut expected_config = UdpMiscSockOpts::default();
        expected_config.time_to_live = Some(64);
        expected_config.type_of_service = Some(20);
        expected_config.netfilter_mark = Some(200);
        assert_eq!(parsed_config, expected_config);
    }

    #[test]
    fn as_udp_misc_sock_opts_err() {
        let yaml = yaml_str!("invalid_key: 10");
        assert!(as_udp_misc_sock_opts(&yaml).is_err());

        let yaml = yaml_str!("time_to_live: 'a string'");
        assert!(as_udp_misc_sock_opts(&yaml).is_err());

        let yaml = yaml_str!("ttl: 4294967296"); // out of range for u32
        assert!(as_udp_misc_sock_opts(&yaml).is_err());

        let yaml = yaml_str!("type_of_service: 'a string'");
        assert!(as_udp_misc_sock_opts(&yaml).is_err());

        let yaml = yaml_str!("tos: 256"); // out of range for u8
        assert!(as_udp_misc_sock_opts(&yaml).is_err());

        let yaml = yaml_str!("netfilter_mark: 'a string'");
        assert!(as_udp_misc_sock_opts(&yaml).is_err());

        let yaml = yaml_str!("mark: -1"); // out of range for u32
        assert!(as_udp_misc_sock_opts(&yaml).is_err());

        let yaml = yaml_str!("a string");
        assert!(as_udp_misc_sock_opts(&yaml).is_err());

        let yaml = Yaml::Boolean(true);
        assert!(as_udp_misc_sock_opts(&yaml).is_err());
    }

    #[test]
    fn as_udp_listen_config_ok() {
        // Integer port
        let yaml = Yaml::Integer(3443);
        let parsed_config = as_udp_listen_config(&yaml).unwrap();
        let mut expected_config = UdpListenConfig::default();
        expected_config.set_port(3443);
        expected_config.check().unwrap();
        assert_eq!(parsed_config, expected_config);

        // String address
        let yaml = yaml_str!("127.0.0.1:8080");
        let parsed_config = as_udp_listen_config(&yaml).unwrap();
        let mut expected_config = UdpListenConfig::default();
        expected_config.set_socket_address("127.0.0.1:8080".parse().unwrap());
        assert_eq!(parsed_config, expected_config);

        // Hash map with IPv4 address
        let yaml = yaml_doc!(
            r#"
                address: 0.0.0.0:5353
                instance_count: 4
                ipv6_only: false
                socket_buffer:
                    receive: 2MB
                    send: 1MB
                socket_misc_opts:
                    ttl: 120
                scale: "75%"
            "#
        );
        let parsed_config = as_udp_listen_config(&yaml).unwrap();
        let mut expected_config = UdpListenConfig::default();
        expected_config.set_socket_address("0.0.0.0:5353".parse().unwrap());
        expected_config.set_instance(4);
        let socket_buffer_yaml = yaml_doc!("receive: 2MB\nsend: 1MB");
        let expected_buf_conf = crate::value::as_socket_buffer_config(&socket_buffer_yaml).unwrap();
        expected_config.set_socket_buffer(expected_buf_conf);
        let mut misc_opts = UdpMiscSockOpts::default();
        misc_opts.time_to_live = Some(120);
        expected_config.set_socket_misc_opts(misc_opts);
        expected_config.set_scale(0.75).unwrap();
        assert_eq!(parsed_config, expected_config);
        assert_eq!(parsed_config.is_ipv6only(), None);

        // Hash map with IPv6 address
        let yaml = yaml_doc!(
            r#"
                addr: "[::]:5353"
                instance: 4
                ipv6only: true
                socket_buffer:
                    recv: 2MB
                    send: 1MB
                socket_misc_opts:
                    ttl: 120
                scale: "75%"
            "#
        );
        let parsed_config = as_udp_listen_config(&yaml).unwrap();
        let mut expected_config = UdpListenConfig::default();
        expected_config.set_socket_address("[::]:5353".parse().unwrap());
        expected_config.set_instance(4);

        #[cfg(not(target_os = "openbsd"))]
        expected_config.set_ipv6_only(true);

        let socket_buffer_yaml = yaml_doc!("receive: 2MB\nsend: 1MB");
        let expected_buf_conf = crate::value::as_socket_buffer_config(&socket_buffer_yaml).unwrap();
        expected_config.set_socket_buffer(expected_buf_conf);
        expected_config.set_socket_misc_opts(misc_opts);
        expected_config.set_scale(0.75).unwrap();
        assert_eq!(parsed_config, expected_config);

        #[cfg(not(target_os = "openbsd"))]
        assert_eq!(parsed_config.is_ipv6only(), Some(true));

        // Scale as fraction
        let yaml = yaml_doc!("addr: 127.0.0.1:1001\nscale: 1/2");
        let parsed_config = as_udp_listen_config(&yaml).unwrap();
        let mut expected_config = UdpListenConfig::default();
        expected_config.set_socket_address("127.0.0.1:1001".parse().unwrap());
        expected_config.set_fraction_scale(1, 2);
        assert_eq!(parsed_config, expected_config);

        // Scale as float
        let yaml = yaml_doc!("addr: 127.0.0.1:1002\nscale: 1.5");
        let parsed_config = as_udp_listen_config(&yaml).unwrap();
        let mut expected_config = UdpListenConfig::default();
        expected_config.set_socket_address("127.0.0.1:1002".parse().unwrap());
        expected_config.set_scale(1.5).unwrap();
        assert_eq!(parsed_config, expected_config);

        // Interface config
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "illumos",
            target_os = "solaris"
        ))]
        {
            use g3_types::net::Interface;

            #[cfg(any(target_os = "linux", target_os = "android"))]
            const LOOPBACK_INTERFACE: &str = "lo";
            #[cfg(not(any(target_os = "linux", target_os = "android")))]
            const LOOPBACK_INTERFACE: &str = "lo0";

            let yaml_str = format!(
                r#"
                address: "0.0.0.0:5353"
                interface: "{}"
            "#,
                LOOPBACK_INTERFACE
            );

            let v = &YamlLoader::load_from_str(&yaml_str).unwrap()[0];
            let parsed_config = as_udp_listen_config(v).unwrap();
            assert_eq!(
                parsed_config.interface(),
                Some(&Interface::from_str(LOOPBACK_INTERFACE).unwrap())
            );
        }
    }

    #[test]
    fn as_udp_listen_config_err() {
        // Invalid scale
        let yaml = yaml_doc!("addr: 127.0.0.1:1005\nscale: 1/a");
        assert!(as_udp_listen_config(&yaml).is_err());

        // Invalid key
        let yaml = yaml_doc!("addr: 127.0.0.1:1006\nfoo: bar");
        assert!(as_udp_listen_config(&yaml).is_err());

        // Invalid address
        let yaml = yaml_doc!("addr: 12345");
        assert!(as_udp_listen_config(&yaml).is_err());

        // Invalid type (boolean)
        let yaml = Yaml::Boolean(true);
        assert!(as_udp_listen_config(&yaml).is_err());

        // Empty hash map
        let yaml = yaml_doc!("{}");
        assert!(as_udp_listen_config(&yaml).is_err());

        // Out-of-range port
        let yaml = Yaml::Integer(65536);
        assert!(as_udp_listen_config(&yaml).is_err());

        // Missing address
        let yaml = yaml_doc!(
            r#"
                instance_count: 4
            "#
        );
        assert!(as_udp_listen_config(&yaml).is_err());

        // Invalid socket_buffer
        let yaml = yaml_doc!(
            r#"
                address: "0.0.0.0:5353"
                socket_buffer:
                    receive: "invalid"
            "#
        );
        assert!(as_udp_listen_config(&yaml).is_err());

        // Invalid socket_misc_opts
        let yaml = yaml_doc!(
            r#"
                address: "0.0.0.0:5353"
                socket_misc_opts:
                    ttl: "invalid"
            "#
        );
        assert!(as_udp_listen_config(&yaml).is_err());
    }
}
