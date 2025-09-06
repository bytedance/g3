/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use super::StreamDumpConfig;

impl StreamDumpConfig {
    pub fn parse_yaml(value: &Yaml) -> anyhow::Result<Self> {
        match value {
            Yaml::Hash(map) => {
                let mut config = StreamDumpConfig::default();

                g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                    "peer" => {
                        config.peer = g3_yaml::value::as_env_sockaddr(v)?;
                        Ok(())
                    }
                    "socket_buffer" => {
                        config.buffer = g3_yaml::value::as_socket_buffer_config(v)
                            .context(format!("invalid socket buffer config value for key {k}"))?;
                        Ok(())
                    }
                    "misc_opts" => {
                        config.opts = g3_yaml::value::as_udp_misc_sock_opts(v)
                            .context(format!("invalid udp misc socket option value for key {k}"))?;
                        Ok(())
                    }
                    "packet_size" => {
                        config.packet_size = g3_yaml::value::as_usize(v)?;
                        Ok(())
                    }
                    "client_side" => {
                        config.client_side = g3_yaml::value::as_bool(v)?;
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid key {k}")),
                })?;

                Ok(config)
            }
            Yaml::String(_) => {
                let config = StreamDumpConfig {
                    peer: g3_yaml::value::as_env_sockaddr(value)?,
                    ..Default::default()
                };
                Ok(config)
            }
            _ => Err(anyhow!(
                "yaml type for 'stream dump config' should be 'map'"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_yaml::{yaml_doc, yaml_str};
    use std::net::SocketAddr;
    use std::str::FromStr;
    use yaml_rust::YamlLoader;

    #[test]
    fn parse_map_ok() {
        let yaml = yaml_doc!(
            r#"
                peer: "127.0.0.1:8080"
                socket_buffer:
                  recv: 65536
                  send: 32768
                misc_opts:
                  time_to_live: 64
                  type_of_service: 32
                packet_size: 1500
                client_side: true
            "#
        );
        let config = StreamDumpConfig::parse_yaml(&yaml).unwrap();
        assert_eq!(config.peer, SocketAddr::from_str("127.0.0.1:8080").unwrap());
        assert_eq!(config.buffer.recv_size(), Some(65536));
        assert_eq!(config.buffer.send_size(), Some(32768));
        assert_eq!(config.opts.time_to_live, Some(64));
        assert_eq!(config.opts.type_of_service, Some(32));
        assert_eq!(config.packet_size, 1500);
        assert!(config.client_side);
    }

    #[test]
    fn parse_map_err() {
        let yaml = yaml_doc!(
            r#"
                invalid_key: "value"
            "#
        );
        assert!(StreamDumpConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                peer: true
            "#
        );
        assert!(StreamDumpConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                socket_buffer: "not_a_map"
            "#
        );
        assert!(StreamDumpConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                misc_opts: "invalid_opts"
            "#
        );
        assert!(StreamDumpConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                packet_size: -100
            "#
        );
        assert!(StreamDumpConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                client_side: "not_a_boolean"
            "#
        );
        assert!(StreamDumpConfig::parse_yaml(&yaml).is_err());
    }

    #[test]
    fn parse_string() {
        // Valid address
        let yaml = yaml_str!("192.168.1.100:3000");
        let config = StreamDumpConfig::parse_yaml(&yaml).unwrap();
        assert_eq!(
            config.peer,
            SocketAddr::from_str("192.168.1.100:3000").unwrap()
        );

        let yaml = yaml_str!("[::1]:8080");
        let config = StreamDumpConfig::parse_yaml(&yaml).unwrap();
        assert_eq!(config.peer, SocketAddr::from_str("[::1]:8080").unwrap());

        // Invalid address
        let yaml = yaml_str!("invalid-address");
        assert!(StreamDumpConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_str!("127.0.0.1"); // Missing port
        assert!(StreamDumpConfig::parse_yaml(&yaml).is_err());
    }

    #[test]
    fn parse_invalid_yaml_types() {
        let yaml = Yaml::Array(vec![]);
        assert!(StreamDumpConfig::parse_yaml(&yaml).is_err());

        let yaml = Yaml::Integer(123);
        assert!(StreamDumpConfig::parse_yaml(&yaml).is_err());

        let yaml = Yaml::Boolean(true);
        assert!(StreamDumpConfig::parse_yaml(&yaml).is_err());

        let yaml = Yaml::Real("1.23".to_string());
        assert!(StreamDumpConfig::parse_yaml(&yaml).is_err());

        let yaml = Yaml::Null;
        assert!(StreamDumpConfig::parse_yaml(&yaml).is_err());
    }
}
