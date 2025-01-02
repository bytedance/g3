/*
 * Copyright 2024 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
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
