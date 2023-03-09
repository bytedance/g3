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
                    "application_audit_ratio" => {
                        let ratio = g3_json::value::as_random_ratio(v)
                            .context(format!("invalid random ratio value for key {k}"))?;
                        self.application_audit_ratio = Some(ratio);
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
