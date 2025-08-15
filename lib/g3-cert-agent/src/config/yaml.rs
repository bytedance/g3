/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use super::CertAgentConfig;

impl CertAgentConfig {
    fn set_query_peer_addr_by_yaml(&mut self, value: &Yaml) -> anyhow::Result<()> {
        let addr = g3_yaml::value::as_env_sockaddr(value)?;
        self.set_query_peer_addr(addr);
        Ok(())
    }

    pub fn parse_yaml(value: &Yaml) -> anyhow::Result<Self> {
        match value {
            Yaml::Hash(map) => {
                let mut config = CertAgentConfig::default();

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
                    "cache_vanish_wait" | "vanish_after_expire" => {
                        let time = g3_yaml::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_cache_vanish_wait(time);
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
                    "protective_cache_ttl" => {
                        let ttl = g3_yaml::value::as_u32(v)?;
                        config.set_protective_cache_ttl(ttl);
                        Ok(())
                    }
                    "maximum_cache_ttl" => {
                        let ttl = g3_yaml::value::as_u32(v)?;
                        config.set_maximum_cache_ttl(ttl);
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid key {k}")),
                })?;

                Ok(config)
            }
            Yaml::String(_) => {
                let mut config = CertAgentConfig::default();
                config
                    .set_query_peer_addr_by_yaml(value)
                    .context("invalid sockaddr str value")?;
                Ok(config)
            }
            _ => Err(anyhow!(
                "yaml type for 'tls cert generator config' should be 'map'"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_yaml::value::as_socket_buffer_config;
    use g3_yaml::yaml_doc;
    use std::time::Duration;
    use yaml_rust::YamlLoader;

    #[test]
    fn parse_yaml_ok() {
        // Full hash configuration
        let yaml = yaml_doc!(
            r#"
            cache_request_batch_count: 20
            cache_request_timeout: 15s
            cache_vanish_wait: 10m
            query_peer_addr: 127.0.0.1:5353
            query_socket_buffer:
                recv: 64KB
                send: 32KB
            query_wait_timeout: 5s
            protective_cache_ttl: 30
            maximum_cache_ttl: 3600
            "#
        );
        let config = CertAgentConfig::parse_yaml(&yaml).unwrap();
        assert_eq!(config.cache_request_batch_count, 20);
        assert_eq!(config.cache_request_timeout, Duration::from_secs(15));
        assert_eq!(config.cache_vanish_wait, Duration::from_secs(600));
        assert_eq!(config.query_peer_addr, "127.0.0.1:5353".parse().unwrap());
        let expected_buffer =
            as_socket_buffer_config(&yaml_doc!("recv: 64KB\nsend: 32KB")).unwrap();
        assert_eq!(config.query_socket_buffer, expected_buffer);
        assert_eq!(config.query_wait_timeout, Duration::from_secs(5));
        assert_eq!(config.protective_cache_ttl, 30);
        assert_eq!(config.maximum_cache_ttl, 3600);

        // Partial configuration with defaults
        let yaml = yaml_doc!(
            r#"
            cache_request_batch_count: 5
            query_peer_addr: 192.168.1.100:2999
            "#
        );

        let config = CertAgentConfig::parse_yaml(&yaml).unwrap();
        assert_eq!(config.cache_request_batch_count, 5);
        assert_eq!(
            config.query_peer_addr,
            "192.168.1.100:2999".parse().unwrap()
        );
        assert_eq!(config.cache_request_timeout, Duration::from_secs(4));
        assert_eq!(config.protective_cache_ttl, 10);

        // String configuration
        let yaml = yaml_doc!("192.168.0.1:5353");
        let config = CertAgentConfig::parse_yaml(&yaml).unwrap();
        assert_eq!(config.query_peer_addr, "192.168.0.1:5353".parse().unwrap());
        assert_eq!(config.cache_request_batch_count, 10); // Default
    }

    #[test]
    fn parse_yaml_err() {
        // Invalid key
        let yaml = yaml_doc!(
            r#"
            invalid_key: value
            "#
        );
        assert!(CertAgentConfig::parse_yaml(&yaml).is_err());

        // Type errors
        let test_cases = vec![
            ("cache_request_batch_count", "not_a_number"),
            ("cache_request_timeout", "invalid_time"),
            ("query_peer_addr", "12345"),   // Invalid address format
            ("protective_cache_ttl", "-5"), // Negative value
            ("maximum_cache_ttl", "string_value"),
        ];

        for (key, value) in test_cases {
            let yaml_str = format!("{key}: {value}");
            let yaml = &YamlLoader::load_from_str(&yaml_str).unwrap()[0];
            assert!(CertAgentConfig::parse_yaml(yaml).is_err());
        }

        // Invalid socket buffer config
        let yaml = yaml_doc!(
            r#"
            query_socket_buffer: invalid_value
            "#
        );
        assert!(CertAgentConfig::parse_yaml(&yaml).is_err());

        // Invalid YAML types
        assert!(CertAgentConfig::parse_yaml(&yaml_rust::Yaml::Boolean(true)).is_err());
        assert!(CertAgentConfig::parse_yaml(&yaml_rust::Yaml::Array(vec![])).is_err());
        assert!(CertAgentConfig::parse_yaml(&yaml_rust::Yaml::Null).is_err());
    }
}
