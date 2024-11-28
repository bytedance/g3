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
