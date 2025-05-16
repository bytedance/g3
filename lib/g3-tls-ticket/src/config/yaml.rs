/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
