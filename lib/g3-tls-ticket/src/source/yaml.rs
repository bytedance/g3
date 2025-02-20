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
