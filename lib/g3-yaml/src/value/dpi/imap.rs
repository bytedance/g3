/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
