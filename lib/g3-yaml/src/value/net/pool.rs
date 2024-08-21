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

use anyhow::{anyhow, Context};
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
            _ => Err(anyhow!("invalid key {k}")),
        })?;
        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'icap connection pool' should be 'map'"
        ))
    }
}
