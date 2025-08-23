/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use super::BlendedRuntimeConfig;

impl BlendedRuntimeConfig {
    pub fn parse_by_yaml_kv(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "thread_number" => {
                let value = g3_yaml::value::as_usize(v)?;
                self.set_thread_number(value);
                Ok(())
            }
            "thread_name" => {
                let name = g3_yaml::value::as_ascii(v)
                    .context(format!("invalid ascii string value for key {k}"))?;
                self.set_thread_name(name.as_str());
                Ok(())
            }
            "thread_stack_size" => {
                let value = g3_yaml::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                self.set_thread_stack_size(value);
                Ok(())
            }
            "max_io_events_per_tick" => {
                let capacity = g3_yaml::value::as_usize(v)?;
                self.set_max_io_events_per_tick(capacity);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_by_yaml_kv_ok() {
        let mut config = BlendedRuntimeConfig::new();

        let yaml = Yaml::Integer(4);
        assert!(config.parse_by_yaml_kv("thread_number", &yaml).is_ok());
        assert_eq!(config.thread_number, Some(4));

        let yaml = Yaml::String("worker_thread".to_string());
        assert!(config.parse_by_yaml_kv("thread_name", &yaml).is_ok());
        assert_eq!(config.thread_name, Some("worker_thread".to_string()));

        let yaml = Yaml::Integer(2048);
        assert!(config.parse_by_yaml_kv("thread_stack_size", &yaml).is_ok());
        assert_eq!(config.thread_stack_size, Some(2048));

        let yaml = Yaml::String("2K".to_string());
        assert!(config.parse_by_yaml_kv("thread_stack_size", &yaml).is_ok());
        assert_eq!(config.thread_stack_size, Some(2000));

        let yaml = Yaml::Integer(512);
        assert!(
            config
                .parse_by_yaml_kv("max_io_events_per_tick", &yaml)
                .is_ok()
        );
        assert_eq!(config.max_io_events_per_tick, Some(512));
    }

    #[test]
    fn parse_by_yaml_kv_err() {
        let mut config = BlendedRuntimeConfig::new();

        let yaml = Yaml::Integer(4);
        assert!(config.parse_by_yaml_kv("invalid_key", &yaml).is_err());

        let yaml = Yaml::String("four".to_string());
        assert!(config.parse_by_yaml_kv("thread_number", &yaml).is_err());

        let yaml = Yaml::Boolean(true);
        assert!(config.parse_by_yaml_kv("thread_name", &yaml).is_err());

        let yaml = Yaml::String("invalid_size".to_string());
        assert!(config.parse_by_yaml_kv("thread_stack_size", &yaml).is_err());

        let yaml = Yaml::String("five".to_string());
        assert!(
            config
                .parse_by_yaml_kv("max_io_events_per_tick", &yaml)
                .is_err()
        );
    }
}
