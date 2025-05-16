/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_dpi::SmtpInterceptionConfig;

pub fn as_smtp_interception_config(value: &Yaml) -> anyhow::Result<SmtpInterceptionConfig> {
    if let Yaml::Hash(map) = value {
        let mut config = SmtpInterceptionConfig::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "greeting_timeout" => {
                config.greeting_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "quit_wait_timeout" => {
                config.quit_wait_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "command_wait_timeout" => {
                config.command_wait_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "response_wait_timeout" => {
                config.response_wait_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "data_initiation_timeout" => {
                config.data_initiation_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "data_termination_timeout" => {
                config.data_termination_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "allow_on_demand_mail_relay" | "allow_odmr" => {
                config.allow_on_demand_mail_relay = crate::value::as_bool(v)?;
                Ok(())
            }
            "allow_data_chunking" => {
                config.allow_data_chunking = crate::value::as_bool(v)?;
                Ok(())
            }
            "allow_burl_data" | "allow_burl" => {
                config.allow_burl_data = crate::value::as_bool(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'smtp interception config' should be 'map'"
        ))
    }
}
