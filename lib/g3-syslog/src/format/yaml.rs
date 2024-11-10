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

use super::SyslogFormatterKind;

impl SyslogFormatterKind {
    pub(crate) fn parse_rfc5424_yaml(value: &Yaml) -> anyhow::Result<Self> {
        let mut enterprise_id = 0i32;
        let mut message_id: Option<String> = None;

        match value {
            Yaml::Hash(map) => {
                g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                    "enterprise_id" => {
                        enterprise_id = g3_yaml::value::as_i32(v)
                            .context(format!("invalid value for key {k}"))?;
                        Ok(())
                    }
                    "message_id" => {
                        message_id = Some(
                            g3_yaml::value::as_string(v)
                                .context(format!("invalid value for key {k}"))?,
                        );
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid key {k}")),
                })?;
                Ok(SyslogFormatterKind::Rfc5424(enterprise_id, message_id))
            }
            Yaml::Integer(i) => {
                enterprise_id =
                    i32::try_from(*i).map_err(|e| anyhow!("invalid enterprise_id: {e}"))?;
                Ok(SyslogFormatterKind::Rfc5424(enterprise_id, message_id))
            }
            Yaml::String(s) => {
                message_id = Some(s.to_string());
                Ok(SyslogFormatterKind::Rfc5424(enterprise_id, message_id))
            }
            _ => Err(anyhow!("invalid yaml value for rfc5424 syslog format")),
        }
    }
}
