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

use g3_ftp_client::{FtpClientConfig, FtpControlConfig, FtpTransferConfig};

fn set_ftp_control_config(value: &Yaml, config: &mut FtpControlConfig) -> anyhow::Result<()> {
    if let Yaml::Hash(map) = value {
        crate::hash::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "max_line_len" | "max_line_length" => {
                config.max_line_len = crate::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                Ok(())
            }
            "max_multi_lines" => {
                config.max_multi_lines = crate::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                Ok(())
            }
            "command_timeout" => {
                config.command_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })
    } else {
        Err(anyhow!("invalid yaml type"))
    }
}

fn set_ftp_transfer_config(value: &Yaml, config: &mut FtpTransferConfig) -> anyhow::Result<()> {
    if let Yaml::Hash(map) = value {
        crate::hash::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "list_max_line_len" | "list_max_line_length" => {
                config.list_max_line_len = crate::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                Ok(())
            }
            "list_max_entries" => {
                config.list_max_entries = crate::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                Ok(())
            }
            "list_all_timeout" => {
                let timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                config.set_list_all_timeout(timeout);
                Ok(())
            }
            "end_wait_timeout" => {
                config.end_wait_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })
    } else {
        Err(anyhow!("invalid yaml type"))
    }
}

pub fn as_ftp_client_config(value: &Yaml) -> anyhow::Result<FtpClientConfig> {
    let mut config = FtpClientConfig::default();
    if let Yaml::Hash(map) = value {
        crate::hash::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "control" => set_ftp_control_config(v, &mut config.control),
            "transfer" => set_ftp_transfer_config(v, &mut config.transfer),
            "connect_timeout" => {
                config.connect_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "greeting_timeout" => {
                config.greeting_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "always_try_epsv" => {
                config.always_try_epsv =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;
    } else {
        return Err(anyhow!("invalid yaml type"));
    }
    Ok(config)
}
