/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::{Arc, OnceLock};

use log::warn;
use yaml_rust::Yaml;

mod config;
pub use config::RegisterConfig;

mod task;
pub use task::RegisterTask;

static PRE_REGISTER_CONFIG: OnceLock<Arc<RegisterConfig>> = OnceLock::new();

pub fn load_pre_config(v: &Yaml) -> anyhow::Result<()> {
    let mut config = RegisterConfig::default();
    config.parse(v)?;
    if PRE_REGISTER_CONFIG.set(Arc::new(config)).is_err() {
        warn!("global register config has already been set");
    }
    Ok(())
}

pub fn get_pre_config() -> Option<Arc<RegisterConfig>> {
    PRE_REGISTER_CONFIG.get().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_yaml::yaml_str;

    #[test]
    fn load_pre_config_ok() {
        let yaml = yaml_str!("127.0.0.1:8080");
        assert!(load_pre_config(&yaml).is_ok());

        let config = get_pre_config().expect("config should be set");
        assert_eq!(config.upstream.to_string(), "127.0.0.1:8080");
    }

    #[test]
    fn load_pre_config_twice() {
        // First load should succeed
        let yaml1 = yaml_str!("127.0.0.1:8080");
        assert!(load_pre_config(&yaml1).is_ok());

        // Second load should succeed but warning
        let yaml2 = yaml_str!("127.0.0.1:9090");
        assert!(load_pre_config(&yaml2).is_ok());

        // Config should remain from first load
        let config = get_pre_config().expect("config should be set");
        assert_eq!(config.upstream.to_string(), "127.0.0.1:8080");
    }

    #[test]
    fn load_pre_config_err() {
        let yaml = Yaml::Array(vec![]);
        assert!(load_pre_config(&yaml).is_err());

        let yaml = Yaml::Integer(123);
        assert!(load_pre_config(&yaml).is_err());
    }
}
