/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use super::TlsTicketConfig;
use crate::source::TicketSourceConfig;

impl TlsTicketConfig {
    pub fn parse_yaml(value: &Yaml, lookup_dir: Option<&Path>) -> anyhow::Result<Self> {
        if let Yaml::Hash(map) = value {
            let mut config = TlsTicketConfig::default();
            g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                "check_interval" => {
                    config.check_interval = g3_yaml::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    Ok(())
                }
                "local_lifetime" => {
                    config.local_lifetime = g3_yaml::value::as_u32(v)?;
                    Ok(())
                }
                "source" => {
                    let source = TicketSourceConfig::parse_yaml(v, lookup_dir).context(format!(
                        "invalid remote tls ticket source config for key {k}"
                    ))?;
                    config.remote_source = Some(source);
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
            Ok(config)
        } else {
            Err(anyhow!(
                "yaml value type for 'tls ticket config' should be 'map'"
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
    fn parse_map_ok() {
        let yaml = yaml_doc!(
            r#"
                check_interval: "600s"
                local_lifetime: 43200
                source:
                  type: "redis"
                  addr: "127.0.0.1:6379"
                  db: 0
                  enc_key: "tls_ticket_enc"
                  dec_set: "tls_ticket_dec"
            "#
        );
        let config = TlsTicketConfig::parse_yaml(&yaml, None).unwrap();
        assert_eq!(config.check_interval, std::time::Duration::from_secs(600));
        assert_eq!(config.local_lifetime, 43200);
        assert!(config.remote_source.is_some());

        let yaml = yaml_doc!(
            r#"
                check_interval: "300s"
            "#
        );
        let config = TlsTicketConfig::parse_yaml(&yaml, None).unwrap();
        assert_eq!(config.check_interval, std::time::Duration::from_secs(300));
        assert_eq!(config.local_lifetime, 12 * 60 * 60);
        assert!(config.remote_source.is_none());
    }

    #[test]
    fn parse_map_err() {
        let yaml = yaml_doc!(
            r#"
                invalid_key: "value"
            "#
        );
        assert!(TlsTicketConfig::parse_yaml(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
                check_interval: "300x"
            "#
        );
        assert!(TlsTicketConfig::parse_yaml(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
                local_lifetime: -1
            "#
        );
        assert!(TlsTicketConfig::parse_yaml(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
                source: "invalid_source"
            "#
        );
        assert!(TlsTicketConfig::parse_yaml(&yaml, None).is_err());
    }

    #[test]
    fn parse_invalid_yaml_types() {
        let yaml = Yaml::Array(vec![]);
        assert!(TlsTicketConfig::parse_yaml(&yaml, None).is_err());

        let yaml = Yaml::Integer(123);
        assert!(TlsTicketConfig::parse_yaml(&yaml, None).is_err());

        let yaml = Yaml::Boolean(true);
        assert!(TlsTicketConfig::parse_yaml(&yaml, None).is_err());

        let yaml = Yaml::Real("1.23".to_string());
        assert!(TlsTicketConfig::parse_yaml(&yaml, None).is_err());

        let yaml = Yaml::Null;
        assert!(TlsTicketConfig::parse_yaml(&yaml, None).is_err());
    }
}
