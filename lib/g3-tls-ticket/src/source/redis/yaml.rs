/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use yaml_rust::yaml;

use super::RedisSourceConfig;
use crate::source::CONFIG_KEY_SOURCE_TYPE;

impl RedisSourceConfig {
    pub(crate) fn parse_yaml_map(
        map: &yaml::Hash,
        lookup_dir: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let mut config = RedisSourceConfig::default();

        g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
            CONFIG_KEY_SOURCE_TYPE => Ok(()),
            "enc_key" => {
                config.enc_key_name = g3_yaml::value::as_string(v)?;
                Ok(())
            }
            "dec_set" => {
                config.dec_set_name = g3_yaml::value::as_string(v)?;
                Ok(())
            }
            _ => config.redis.set_by_yaml_kv(k, v, lookup_dir),
        })?;

        config.check()?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_yaml::yaml_doc;
    use yaml_rust::YamlLoader;

    #[test]
    fn parse_yaml_map_ok() {
        let yaml = yaml_doc!(
            r#"
                type: "redis"
                enc_key: "tls_ticket_enc"
                dec_set: "tls_ticket_dec"
                addr: "127.0.0.1:6379"
                db: 0
            "#
        );
        let config = RedisSourceConfig::parse_yaml_map(yaml.as_hash().unwrap(), None).unwrap();
        assert_eq!(config.enc_key_name, "tls_ticket_enc");
        assert_eq!(config.dec_set_name, "tls_ticket_dec");
    }

    #[test]
    fn parse_yaml_map_err() {
        // Missing enc_key
        let yaml = yaml_doc!(
            r#"
                type: "redis"
                dec_set: "tls_ticket_dec"
                addr: "127.0.0.1:6379"
            "#
        );
        assert!(RedisSourceConfig::parse_yaml_map(yaml.as_hash().unwrap(), None).is_err());

        // Missing dec_set
        let yaml = yaml_doc!(
            r#"
                type: "redis"
                enc_key: "tls_ticket_enc"
                addr: "127.0.0.1:6379"
            "#
        );
        assert!(RedisSourceConfig::parse_yaml_map(yaml.as_hash().unwrap(), None).is_err());

        // Empty enc_key
        let yaml = yaml_doc!(
            r#"
                type: "redis"
                enc_key: ""
                dec_set: "tls_ticket_dec"
                addr: "127.0.0.1:6379"
            "#
        );
        assert!(RedisSourceConfig::parse_yaml_map(yaml.as_hash().unwrap(), None).is_err());

        // Empty dec_set
        let yaml = yaml_doc!(
            r#"
                type: "redis"
                enc_key: "tls_ticket_enc"
                dec_set: ""
                addr: "127.0.0.1:6379"
            "#
        );
        assert!(RedisSourceConfig::parse_yaml_map(yaml.as_hash().unwrap(), None).is_err());
    }
}
