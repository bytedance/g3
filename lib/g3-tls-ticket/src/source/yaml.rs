/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::anyhow;
use yaml_rust::Yaml;

use super::{CONFIG_KEY_SOURCE_TYPE, TicketSourceConfig};

impl TicketSourceConfig {
    pub(crate) fn parse_yaml(value: &Yaml, lookup_dir: Option<&Path>) -> anyhow::Result<Self> {
        if let Yaml::Hash(map) = value {
            let source_type = g3_yaml::hash_get_required_str(map, CONFIG_KEY_SOURCE_TYPE)?;

            match g3_yaml::key::normalize(source_type).as_str() {
                "redis" => {
                    let source = super::RedisSourceConfig::parse_yaml_map(map, lookup_dir)?;
                    Ok(TicketSourceConfig::Redis(source))
                }
                _ => Err(anyhow!("unsupported source type {source_type}")),
            }
        } else {
            Err(anyhow!(
                "yaml value type for tls ticket source should be 'map'"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_yaml::yaml_doc;
    use yaml_rust::YamlLoader;

    #[test]
    fn parse_yaml_ok() {
        let yaml = yaml_doc!(
            r#"
                type: "redis"
                enc_key: "tls_ticket_enc"
                dec_set: "tls_ticket_dec"
                addr: "127.0.0.1:6379"
            "#
        );
        let config = TicketSourceConfig::parse_yaml(&yaml, None).unwrap();
        assert!(matches!(config, TicketSourceConfig::Redis(_)));

        let TicketSourceConfig::Redis(redis_config) = config;
        assert!(redis_config.build().is_ok());
    }

    #[test]
    fn parse_yaml_err() {
        let yaml = yaml_doc!(
            r#"
                type: "unsupported"
                addr: "127.0.0.1:6379"
            "#
        );
        assert!(TicketSourceConfig::parse_yaml(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
                addr: "127.0.0.1:6379"
            "#
        );
        assert!(TicketSourceConfig::parse_yaml(&yaml, None).is_err());

        let yaml = Yaml::Integer(123);
        assert!(TicketSourceConfig::parse_yaml(&yaml, None).is_err());

        let yaml = Yaml::Boolean(true);
        assert!(TicketSourceConfig::parse_yaml(&yaml, None).is_err());

        let yaml = Yaml::Array(vec![]);
        assert!(TicketSourceConfig::parse_yaml(&yaml, None).is_err());

        let yaml = Yaml::Real("1.23".to_string());
        assert!(TicketSourceConfig::parse_yaml(&yaml, None).is_err());

        let yaml = Yaml::Null;
        assert!(TicketSourceConfig::parse_yaml(&yaml, None).is_err());
    }
}
