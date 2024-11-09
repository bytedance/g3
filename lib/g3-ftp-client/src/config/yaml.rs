/*
 * Copyright 2023 ByteDance and/or its affiliates.
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

use super::{FtpClientConfig, FtpControlConfig, FtpTransferConfig};

impl FtpControlConfig {
    pub fn parse_yaml(value: &Yaml) -> anyhow::Result<Self> {
        if let Yaml::Hash(map) = value {
            let mut config = FtpControlConfig::default();
            g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                "max_line_len" | "max_line_length" => {
                    config.max_line_len = g3_yaml::humanize::as_usize(v)
                        .context(format!("invalid humanize usize value for key {k}"))?;
                    Ok(())
                }
                "max_multi_lines" => {
                    config.max_multi_lines = g3_yaml::value::as_usize(v)
                        .context(format!("invalid usize value for key {k}"))?;
                    Ok(())
                }
                "command_timeout" => {
                    config.command_timeout = g3_yaml::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
            Ok(config)
        } else {
            Err(anyhow!("invalid yaml type"))
        }
    }
}

impl FtpTransferConfig {
    pub fn parse_yaml(value: &Yaml) -> anyhow::Result<Self> {
        if let Yaml::Hash(map) = value {
            let mut config = FtpTransferConfig::default();
            g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                "list_max_line_len" | "list_max_line_length" => {
                    config.list_max_line_len = g3_yaml::humanize::as_usize(v)
                        .context(format!("invalid humanize usize value for key {k}"))?;
                    Ok(())
                }
                "list_max_entries" => {
                    config.list_max_entries = g3_yaml::value::as_usize(v)
                        .context(format!("invalid usize value for key {k}"))?;
                    Ok(())
                }
                "list_all_timeout" => {
                    let timeout = g3_yaml::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    config.set_list_all_timeout(timeout);
                    Ok(())
                }
                "end_wait_timeout" => {
                    config.end_wait_timeout = g3_yaml::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
            Ok(config)
        } else {
            Err(anyhow!("invalid yaml type"))
        }
    }
}

impl FtpClientConfig {
    pub fn parse_yaml(value: &Yaml) -> anyhow::Result<Self> {
        if let Yaml::Hash(map) = value {
            let mut config = FtpClientConfig::default();
            g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                "control" => {
                    config.control = FtpControlConfig::parse_yaml(v).context(format!(
                        "invalid ftp control connection config value for key {k}"
                    ))?;
                    Ok(())
                }
                "transfer" => {
                    config.transfer = FtpTransferConfig::parse_yaml(v).context(format!(
                        "invalid ftp transfer connection config value for key {k}"
                    ))?;
                    Ok(())
                }
                "connect_timeout" => {
                    config.connect_timeout = g3_yaml::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    Ok(())
                }
                "greeting_timeout" => {
                    config.greeting_timeout = g3_yaml::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    Ok(())
                }
                "always_try_epsv" => {
                    config.always_try_epsv = g3_yaml::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
            Ok(config)
        } else {
            Err(anyhow!("invalid yaml type"))
        }
    }
}
