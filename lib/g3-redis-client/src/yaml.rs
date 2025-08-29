/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use super::RedisClientConfigBuilder;

impl RedisClientConfigBuilder {
    pub fn set_by_yaml_kv(
        &mut self,
        k: &str,
        v: &Yaml,
        lookup_dir: Option<&Path>,
    ) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "addr" | "address" => {
                let addr = g3_yaml::value::as_upstream_addr(v, crate::REDIS_DEFAULT_PORT)
                    .context(format!("invalid upstream address value for key {k}"))?;
                self.set_addr(addr);
                Ok(())
            }
            "tls" | "tls_client" => {
                let tls = g3_yaml::value::as_rustls_client_config_builder(v, lookup_dir).context(
                    format!("invalid rustls tls client config value for key {k}"),
                )?;
                self.set_tls_client(tls);
                Ok(())
            }
            "tls_name" => {
                let name = g3_yaml::value::as_rustls_server_name(v)
                    .context(format!("invalid rustls server name value for key {k}"))?;
                self.set_tls_name(name);
                Ok(())
            }
            "db" => {
                let db =
                    g3_yaml::value::as_i64(v).context(format!("invalid int value for key {k}"))?;
                self.set_db(db);
                Ok(())
            }
            "username" => {
                let username = g3_yaml::value::as_string(v)?;
                self.set_username(username);
                Ok(())
            }
            "password" => {
                let password = g3_yaml::value::as_string(v)?;
                self.set_password(password);
                Ok(())
            }
            "connect_timeout" => {
                let timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                self.set_connect_timeout(timeout);
                Ok(())
            }
            "response_timeout" | "read_timeout" => {
                let timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                self.set_response_timeout(timeout);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {}", k)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_types::net::{Host, RustlsClientConfigBuilder, UpstreamAddr};
    use g3_yaml::yaml_doc;
    use rustls_pki_types::ServerName;
    use std::net::IpAddr;
    use std::str::FromStr;
    use std::time::Duration;
    use yaml_rust::YamlLoader;

    #[test]
    fn set_by_yaml_kv_ok() {
        let mut builder = RedisClientConfigBuilder::default();
        let yaml = yaml_doc!(
            r#"
                addr: "127.0.0.1:6380"
                tls:
                  no_session_cache: true
                  disable_sni: true
                  max_fragment_size: 1024
                tls_name: "redis.example.com"
                db: 5
                username: "test_user"
                password: "test_pass"
                connect_timeout: "10s"
                response_timeout: "5s"
                "#
        );
        for (k, v) in yaml.as_hash().unwrap().iter() {
            builder
                .set_by_yaml_kv(k.as_str().unwrap(), v, None)
                .unwrap();
        }

        let mut expected = RedisClientConfigBuilder::default();
        expected.set_addr(UpstreamAddr::new(
            Host::Ip(IpAddr::from_str("127.0.0.1").unwrap()),
            6380,
        ));
        let mut tls = RustlsClientConfigBuilder::default();
        tls.set_no_session_cache();
        tls.set_disable_sni();
        tls.set_max_fragment_size(1024);
        expected.set_tls_client(tls);
        expected.set_tls_name(ServerName::try_from("redis.example.com").unwrap());
        expected.set_db(5);
        expected.set_username("test_user".to_string());
        expected.set_password("test_pass".to_string());
        expected.set_connect_timeout(Duration::from_secs(10));
        expected.set_response_timeout(Duration::from_secs(5));

        assert_eq!(builder, expected);
    }

    #[test]
    fn set_by_yaml_kv_err() {
        let mut builder = RedisClientConfigBuilder::default();
        let yaml = yaml_doc!(
            r#"
                invalid_key: "value"
                address: false
                tls_client: "invalid"
                tls_name: "invalid..name"
                db: -1.5
                username: true
                password: null
                connect_timeout: "-10s"
                read_timeout: "5xs"
                "#
        );
        for (k, v) in yaml.as_hash().unwrap().iter() {
            assert!(
                builder
                    .set_by_yaml_kv(k.as_str().unwrap(), v, None)
                    .is_err()
            );
        }
    }
}
