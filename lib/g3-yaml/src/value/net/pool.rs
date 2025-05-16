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
