/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use anyhow::anyhow;
use yaml_rust::Yaml;

#[derive(Debug, Clone, Copy)]
pub(crate) struct AsyncJobBackendConfig {
    pub(crate) async_op_timeout: Duration,
}

impl Default for AsyncJobBackendConfig {
    fn default() -> Self {
        AsyncJobBackendConfig {
            async_op_timeout: Duration::from_secs(1),
        }
    }
}

impl AsyncJobBackendConfig {
    pub(super) fn parse_yaml(value: &Yaml) -> anyhow::Result<Self> {
        if let Yaml::Hash(map) = value {
            let mut config = AsyncJobBackendConfig::default();
            g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                "async_op_timeout" => {
                    config.async_op_timeout = g3_yaml::humanize::as_duration(v)?;
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
            Ok(config)
        } else {
            Err(anyhow!(
                "yaml value type for `openssl async job backend` should be `map`"
            ))
        }
    }
}
