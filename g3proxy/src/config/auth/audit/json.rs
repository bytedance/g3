/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use serde_json::Value;

use super::UserAuditConfig;

impl UserAuditConfig {
    pub(crate) fn parse_json(&mut self, v: &Value) -> anyhow::Result<()> {
        if let Value::Object(map) = v {
            for (k, v) in map {
                match g3_json::key::normalize(k).as_str() {
                    "enable_protocol_inspection" => {
                        self.enable_protocol_inspection = g3_json::value::as_bool(v)
                            .context(format!("invalid bool value for key {k}"))?;
                    }
                    "prohibit_unknown_protocol" => {
                        self.prohibit_unknown_protocol = g3_json::value::as_bool(v)
                            .context(format!("invalid bool value for key {k}"))?;
                    }
                    "prohibit_timeout_protocol" => {
                        self.prohibit_timeout_protocol = g3_json::value::as_bool(v)
                            .context(format!("invalid bool value for key {k}"))?;
                    }
                    "task_audit_ratio" | "application_audit_ratio" => {
                        let ratio = g3_json::value::as_random_ratio(v)
                            .context(format!("invalid random ratio value for key {k}"))?;
                        self.task_audit_ratio = Some(ratio);
                    }
                    _ => return Err(anyhow!("invalid key {k}")),
                }
            }
            Ok(())
        } else {
            Err(anyhow!(
                "json value type for 'user audit config' should be 'map'"
            ))
        }
    }
}
