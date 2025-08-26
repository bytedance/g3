/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use super::FluentdClientConfig;

impl FluentdClientConfig {
    pub fn parse_yaml(value: &Yaml, lookup_dir: Option<&Path>) -> anyhow::Result<Self> {
        match value {
            Yaml::Hash(map) => {
                let mut config = FluentdClientConfig::default();

                g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                    "address" | "addr" => {
                        let addr = g3_yaml::value::as_env_sockaddr(v)?;
                        config.set_server_addr(addr);
                        Ok(())
                    }
                    "bind_ip" | "bind" => {
                        let ip = g3_yaml::value::as_ipaddr(v)?;
                        config.set_bind_ip(ip);
                        Ok(())
                    }
                    "shared_key" => {
                        let key = g3_yaml::value::as_string(v)?;
                        config.set_shared_key(key);
                        Ok(())
                    }
                    "username" => {
                        let name = g3_yaml::value::as_string(v)?;
                        config.set_username(name);
                        Ok(())
                    }
                    "password" => {
                        let pass = g3_yaml::value::as_string(v)?;
                        config.set_password(pass);
                        Ok(())
                    }
                    "hostname" => {
                        let hostname = g3_yaml::value::as_string(v)?;
                        config.set_hostname(hostname);
                        Ok(())
                    }
                    "tcp_keepalive" => {
                        let keepalive = g3_yaml::value::as_tcp_keepalive_config(v)
                            .context(format!("invalid tcp keepalive config value for key {k}"))?;
                        config.set_tcp_keepalive(keepalive);
                        Ok(())
                    }
                    "tls" | "tls_client" => {
                        let tls_config =
                            g3_yaml::value::as_to_one_openssl_tls_client_config_builder(
                                v, lookup_dir,
                            )
                            .context(format!(
                                "invalid openssl tls client config value for key {k}"
                            ))?;
                        config
                            .set_tls_client(tls_config)
                            .context("failed to set tls client config")?;
                        Ok(())
                    }
                    "tls_name" => {
                        let tls_name = g3_yaml::value::as_host(v)
                            .context(format!("invalid tls server name value for key {k}"))?;
                        config.set_tls_name(tls_name);
                        Ok(())
                    }
                    "connect_timeout" => {
                        let timeout = g3_yaml::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_connect_timeout(timeout);
                        Ok(())
                    }
                    "connect_delay" => {
                        let delay = g3_yaml::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_connect_delay(delay);
                        Ok(())
                    }
                    "write_timeout" => {
                        let timeout = g3_yaml::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_write_timeout(timeout);
                        Ok(())
                    }
                    "flush_interval" => {
                        let interval = g3_yaml::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_flush_interval(interval);
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid key {k}")),
                })?;

                Ok(config)
            }
            Yaml::String(_) => {
                let addr = g3_yaml::value::as_env_sockaddr(value)?;
                let config = FluentdClientConfig::new(addr);
                Ok(config)
            }
            Yaml::Null => {
                let config = FluentdClientConfig::default();
                Ok(config)
            }
            _ => Err(anyhow!(
                "yaml value type for 'FluentdConfig' should be 'map'"
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
    fn parse_map() {
        let yaml = yaml_doc!(
            r#"
                address: "192.168.1.1:24224"
                bind_ip: "10.0.0.1"
                shared_key: "test_key"
                username: "test_user"
                password: "test_pass"
                hostname: "test_host"
                tcp_keepalive:
                  idle_time: 60
                  probe_interval: 10
                  probe_count: 3
                tls_client:
                  handshake_timeout: 10s
                tls_name: "example.com"
                connect_timeout: "5s"
                connect_delay: "1s"
                write_timeout: "500ms"
                flush_interval: "100ms"
            "#
        );
        let config = FluentdClientConfig::parse_yaml(&yaml, None).unwrap();
        assert_eq!(config.server_addr, "192.168.1.1:24224".parse().unwrap());
        assert_eq!(config.bind.ip().unwrap().to_string(), "10.0.0.1");
        assert_eq!(config.shared_key, "test_key");
        assert_eq!(config.username, "test_user");
        assert_eq!(config.password, "test_pass");
        assert_eq!(config.hostname, "test_host");
        assert_eq!(
            config.tcp_keepalive.idle_time(),
            std::time::Duration::from_secs(60)
        );
        assert_eq!(
            config.tcp_keepalive.probe_interval(),
            Some(std::time::Duration::from_secs(10))
        );
        assert_eq!(config.tcp_keepalive.probe_count(), Some(3));
        assert_eq!(
            config.tls_client.unwrap().handshake_timeout,
            std::time::Duration::from_secs(10)
        );
        assert_eq!(config.tls_name.unwrap().to_string(), "example.com");
        assert_eq!(config.connect_timeout, std::time::Duration::from_secs(5));
        assert_eq!(config.connect_delay, std::time::Duration::from_secs(1));
        assert_eq!(config.write_timeout, std::time::Duration::from_millis(500));
        assert_eq!(config.flush_interval, std::time::Duration::from_millis(100));

        let yaml = yaml_doc!(
            r#"
                addr: "127.0.0.1:8080"
                bind: "192.168.1.1"
                tls:
                  handshake_timeout: 15s
            "#
        );
        let config = FluentdClientConfig::parse_yaml(&yaml, None).unwrap();
        assert_eq!(config.server_addr, "127.0.0.1:8080".parse().unwrap());
        assert_eq!(config.bind.ip().unwrap().to_string(), "192.168.1.1");
        assert_eq!(
            config.tls_client.unwrap().handshake_timeout,
            std::time::Duration::from_secs(15)
        );

        let yaml = yaml_doc!(
            r#"
                invalid_key: "value"
            "#
        );
        assert!(FluentdClientConfig::parse_yaml(&yaml, None).is_err());

        // Invalid address
        let yaml = yaml_doc!(
            r#"
                address: 12345
            "#
        );
        assert!(FluentdClientConfig::parse_yaml(&yaml, None).is_err());

        // Invalid bind IP
        let yaml = yaml_doc!(
            r#"
                bind_ip: "invalid_ip"
            "#
        );
        assert!(FluentdClientConfig::parse_yaml(&yaml, None).is_err());

        // Invalid duration
        let yaml = yaml_doc!(
            r#"
                connect_timeout: "invalid_duration"
            "#
        );
        assert!(FluentdClientConfig::parse_yaml(&yaml, None).is_err());

        // Invalid TLS config
        let yaml = yaml_doc!(
            r#"
                tls: "invalid_tls_config"
            "#
        );
        assert!(FluentdClientConfig::parse_yaml(&yaml, None).is_err());
    }

    #[test]
    fn parse_string() {
        // Valid address
        let yaml = yaml_str!("127.0.0.1:8080");
        let config = FluentdClientConfig::parse_yaml(&yaml, None).unwrap();
        assert_eq!(config.server_addr.port(), 8080);
        assert_eq!(config.server_addr.ip().to_string(), "127.0.0.1");

        // Invalid address
        let yaml = yaml_str!("invalid-address");
        assert!(FluentdClientConfig::parse_yaml(&yaml, None).is_err());
    }

    #[test]
    fn parse_null() {
        let config = FluentdClientConfig::parse_yaml(&Yaml::Null, None).unwrap();
        assert_eq!(config.server_addr.port(), 24224);
        assert!(config.shared_key.is_empty());
        assert!(config.username.is_empty());
        assert!(config.password.is_empty());
    }

    #[test]
    fn parse_invalid_yaml_types() {
        // Array
        let yaml = Yaml::Array(vec![]);
        assert!(FluentdClientConfig::parse_yaml(&yaml, None).is_err());

        // Integer
        let yaml = Yaml::Integer(123);
        assert!(FluentdClientConfig::parse_yaml(&yaml, None).is_err());

        // Boolean
        let yaml = Yaml::Boolean(true);
        assert!(FluentdClientConfig::parse_yaml(&yaml, None).is_err());

        // Float
        let yaml = Yaml::Real("1.23".to_string());
        assert!(FluentdClientConfig::parse_yaml(&yaml, None).is_err());
    }
}
