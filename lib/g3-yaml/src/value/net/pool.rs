/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::net::ConnectionPoolConfig;

pub fn as_connection_pool_config(value: &Yaml) -> anyhow::Result<ConnectionPoolConfig> {
    if let Yaml::Hash(map) = value {
        let mut config = ConnectionPoolConfig::default();
        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "check_interval" => {
                let interval = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                config.set_check_interval(interval);
                Ok(())
            }
            "max_idle_count" => {
                let count = crate::value::as_usize(v)?;
                config.set_max_idle_count(count);
                Ok(())
            }
            "min_idle_count" => {
                let count = crate::value::as_usize(v)?;
                config.set_min_idle_count(count);
                Ok(())
            }
            "idle_timeout" => {
                let timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                config.set_idle_timeout(timeout);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;
        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'icap connection pool' should be 'map'"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    #[test]
    fn as_connection_pool_config_ok() {
        let yaml = yaml_doc!(
            r#"
                check_interval: 30s
                max_idle_count: 100
                min_idle_count: 10
                idle_timeout: 5m
            "#
        );
        let config = as_connection_pool_config(&yaml).unwrap();
        assert_eq!(config.check_interval().as_secs(), 30);
        assert_eq!(config.max_idle_count(), 100);
        assert_eq!(config.min_idle_count(), 10);
        assert_eq!(config.idle_timeout().as_secs(), 300);

        let yaml = yaml_doc!(
            r#"
                max_idle_count: 50
                idle_timeout: 1h
            "#
        );
        let config = as_connection_pool_config(&yaml).unwrap();
        assert_eq!(config.max_idle_count(), 50);
        assert_eq!(config.idle_timeout().as_secs(), 3600);

        let yaml = yaml_doc!(
            r#"
                CHECK_INTERVAL: 15s
                MAX_IDLE_COUNT: 200
            "#
        );
        let config = as_connection_pool_config(&yaml).unwrap();
        assert_eq!(config.check_interval().as_secs(), 15);
        assert_eq!(config.max_idle_count(), 200);

        let yaml = yaml_doc!(
            r#"
                min_idle_count: 0
                idle_timeout: 0s
            "#
        );
        let config = as_connection_pool_config(&yaml).unwrap();
        assert_eq!(config.min_idle_count(), 0);
        assert_eq!(config.idle_timeout().as_secs(), 0);
    }

    #[test]
    fn as_connection_pool_config_err() {
        let yaml = yaml_doc!(
            r#"
                invalid_key: value
            "#
        );
        let result = as_connection_pool_config(&yaml);
        assert!(result.is_err());

        let yaml = yaml_doc!(
            r#"
                - array_item
            "#
        );
        let result = as_connection_pool_config(&yaml);
        assert!(result.is_err());

        let yaml = yaml_doc!(
            r#"
                "just a string"
            "#
        );
        let result = as_connection_pool_config(&yaml);
        assert!(result.is_err());

        let yaml = yaml_doc!(
            r#"
                check_interval: 30seconds
            "#
        );
        let result = as_connection_pool_config(&yaml);
        assert!(result.is_err());

        let yaml = yaml_doc!(
            r#"
                max_idle_count: -10
            "#
        );
        let result = as_connection_pool_config(&yaml);
        assert!(result.is_err());
    }
}
