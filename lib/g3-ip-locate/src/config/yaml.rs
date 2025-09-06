/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use crate::IpLocateServiceConfig;

impl IpLocateServiceConfig {
    fn set_query_peer_addr_by_yaml(&mut self, value: &Yaml) -> anyhow::Result<()> {
        let addr = g3_yaml::value::as_env_sockaddr(value)?;
        self.set_query_peer_addr(addr);
        Ok(())
    }

    pub fn parse_yaml(value: &Yaml) -> anyhow::Result<Self> {
        match value {
            Yaml::Hash(map) => {
                let mut config = IpLocateServiceConfig::default();

                g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                    "cache_request_batch_count" => {
                        let count = g3_yaml::value::as_usize(v)?;
                        config.set_cache_request_batch_count(count);
                        Ok(())
                    }
                    "cache_request_timeout" => {
                        let time = g3_yaml::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_cache_request_timeout(time);
                        Ok(())
                    }
                    "query_peer_addr" => {
                        config
                            .set_query_peer_addr_by_yaml(v)
                            .context(format!("invalid sockaddr str value for key {k}"))?;
                        Ok(())
                    }
                    "query_socket_buffer" => {
                        let buf_config = g3_yaml::value::as_socket_buffer_config(v)
                            .context(format!("invalid socket buffer config value for key {k}"))?;
                        config.set_query_socket_buffer(buf_config);
                        Ok(())
                    }
                    "query_wait_timeout" => {
                        let time = g3_yaml::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_query_wait_timeout(time);
                        Ok(())
                    }
                    "default_expire_ttl" => {
                        let ttl = g3_yaml::value::as_u32(v)?;
                        config.set_default_expire_ttl(ttl);
                        Ok(())
                    }
                    "maximum_expire_ttl" => {
                        let ttl = g3_yaml::value::as_u32(v)?;
                        config.set_maximum_expire_ttl(ttl);
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid key {k}")),
                })?;

                Ok(config)
            }
            Yaml::String(_) => {
                let mut config = IpLocateServiceConfig::default();
                config
                    .set_query_peer_addr_by_yaml(value)
                    .context("invalid sockaddr str value")?;
                Ok(config)
            }
            _ => Err(anyhow!(
                "yaml type for 'ip location service config' should be 'map'"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_yaml::{yaml_doc, yaml_str};
    use yaml_rust::YamlLoader;

    #[test]
    fn parse_map_ok() {
        let yaml = yaml_doc!(
            r#"
                cache_request_batch_count: 20
                cache_request_timeout: "5s"
                query_peer_addr: "192.168.1.1:2888"
                query_socket_buffer:
                  recv: 65536
                  send: 32768
                query_wait_timeout: "2s"
                default_expire_ttl: 30
                maximum_expire_ttl: 600
            "#
        );
        let config = IpLocateServiceConfig::parse_yaml(&yaml).unwrap();
        assert_eq!(config.cache_request_batch_count, 20);
        assert_eq!(
            config.cache_request_timeout,
            std::time::Duration::from_secs(5)
        );
        assert_eq!(config.query_peer_addr, "192.168.1.1:2888".parse().unwrap());
        assert_eq!(config.query_socket_buffer.recv_size(), Some(65536));
        assert_eq!(config.query_socket_buffer.send_size(), Some(32768));
        assert_eq!(config.query_wait_timeout, std::time::Duration::from_secs(2));
        assert_eq!(config.default_expire_ttl, 30);
        assert_eq!(config.maximum_expire_ttl, 600);
    }

    #[test]
    fn parse_map_err() {
        let yaml = yaml_doc!(
            r#"
                invalid_key: "value"
            "#
        );
        assert!(IpLocateServiceConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                cache_request_batch_count: -1
            "#
        );
        assert!(IpLocateServiceConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                cache_request_timeout: "5x"
            "#
        );
        assert!(IpLocateServiceConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                query_peer_addr: "invalid_address"
            "#
        );
        assert!(IpLocateServiceConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                query_socket_buffer:
                  recv: -1024
            "#
        );
        assert!(IpLocateServiceConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                query_wait_timeout: "-1s"
            "#
        );
        assert!(IpLocateServiceConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                default_expire_ttl: -10
            "#
        );
        assert!(IpLocateServiceConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                maximum_expire_ttl: NaN
            "#
        );
        assert!(IpLocateServiceConfig::parse_yaml(&yaml).is_err());
    }

    #[test]
    fn parse_string() {
        // Valid address
        let yaml = yaml_str!("127.0.0.1:3000");
        let config = IpLocateServiceConfig::parse_yaml(&yaml).unwrap();
        assert_eq!(config.query_peer_addr, "127.0.0.1:3000".parse().unwrap());

        // Invalid address
        let yaml = yaml_str!("invalid-address");
        assert!(IpLocateServiceConfig::parse_yaml(&yaml).is_err());
    }

    #[test]
    fn parse_invalid_yaml_types() {
        let yaml = Yaml::Array(vec![]);
        assert!(IpLocateServiceConfig::parse_yaml(&yaml).is_err());

        let yaml = Yaml::Integer(123);
        assert!(IpLocateServiceConfig::parse_yaml(&yaml).is_err());

        let yaml = Yaml::Boolean(true);
        assert!(IpLocateServiceConfig::parse_yaml(&yaml).is_err());

        let yaml = Yaml::Real("1.23".to_string());
        assert!(IpLocateServiceConfig::parse_yaml(&yaml).is_err());

        let yaml = Yaml::Null;
        assert!(IpLocateServiceConfig::parse_yaml(&yaml).is_err());
    }
}
