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

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_dpi::ImapInterceptionConfig;

pub fn as_imap_interception_config(value: &Yaml) -> anyhow::Result<ImapInterceptionConfig> {
    if let Yaml::Hash(map) = value {
        let mut config = ImapInterceptionConfig::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "greeting_timeout" => {
                config.greeting_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "authenticate_timeout" => {
                config.authenticate_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "logout_wait_timeout" => {
                config.logout_wait_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "command_line_max_size" => {
                config.command_line_max_size = crate::value::as_usize(v)?;
                Ok(())
            }
            "response_line_max_size" => {
                config.response_line_max_size = crate::value::as_usize(v)?;
                Ok(())
            }
            "forward_max_idle_count" => {
                config.forward_max_idle_count = crate::value::as_usize(v)?;
                Ok(())
            }
            "transfer_max_idle_count" => {
                config.transfer_max_idle_count = crate::value::as_usize(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'imap interception config' should be 'map'"
        ))
    }
}
