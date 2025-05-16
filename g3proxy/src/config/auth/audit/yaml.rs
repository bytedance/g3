/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use super::UserAuditConfig;

impl UserAuditConfig {
    pub(crate) fn parse_yaml(&mut self, v: &Yaml) -> anyhow::Result<()> {
        if let Yaml::Hash(map) = v {
            g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                "enable_protocol_inspection" => {
                    self.enable_protocol_inspection = g3_yaml::value::as_bool(v)?;
                    Ok(())
                }
                "prohibit_unknown_protocol" => {
                    self.prohibit_unknown_protocol = g3_yaml::value::as_bool(v)?;
                    Ok(())
                }
                "prohibit_timeout_protocol" => {
                    self.prohibit_timeout_protocol = g3_yaml::value::as_bool(v)?;
                    Ok(())
                }
                "task_audit_ratio" | "application_audit_ratio" => {
                    let ratio = g3_yaml::value::as_random_ratio(v)
                        .context(format!("invalid random ratio value for key {k}"))?;
                    self.task_audit_ratio = Some(ratio);
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })
        } else {
            Err(anyhow!(
                "yaml value type for 'user audit config' should 'map'"
            ))
        }
    }
}
