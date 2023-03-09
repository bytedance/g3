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

use g3_dpi::{ProtocolInspectionConfig, ProtocolInspectionSizeLimit};

pub fn parse_inspect_size_limit(
    config: &mut ProtocolInspectionSizeLimit,
    value: &Yaml,
) -> anyhow::Result<()> {
    if let Yaml::Hash(map) = value {
        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "ftp_greeting_msg" | "ftp_server_greeting_msg" => {
                let size = crate::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                config.set_ftp_server_greeting_msg(size);
                Ok(())
            }
            "http_request_uri" | "http_client_request_uri" => {
                let size = crate::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                config.set_http_client_request_uri(size);
                Ok(())
            }
            "imap_greeting_msg" | "imap_server_greeting_msg" => {
                let size = crate::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                config.set_imap_server_greeting_msg(size);
                Ok(())
            }
            "nats_info_line" | "nats_server_info_line" => {
                let size = crate::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                config.set_nats_server_info_line(size);
                Ok(())
            }
            "smtp_greeting_msg" | "smtp_server_greeting_msg" => {
                let size = crate::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                config.set_smtp_server_greeting_msg(size);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })
    } else {
        Err(anyhow!(
            "yaml value type for 'inspect size limit' should be 'map'"
        ))
    }
}

pub fn as_protocol_inspection_config(value: &Yaml) -> anyhow::Result<ProtocolInspectionConfig> {
    if let Yaml::Hash(map) = value {
        let mut config = ProtocolInspectionConfig::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "data0_buffer_size" => {
                let size = crate::humanize::as_usize(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                config.set_data0_buffer_size(size);
                Ok(())
            }
            "inspect_max_depth" => {
                let depth = crate::value::as_usize(v)?;
                config.set_max_depth(depth);
                Ok(())
            }
            "data0_wait_timeout" => {
                let value = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                config.set_data0_wait_timeout(value);
                Ok(())
            }
            "data0_read_timeout" => {
                let value = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                config.set_data0_read_timeout(value);
                Ok(())
            }
            "data0_size_limit" => parse_inspect_size_limit(config.size_limit_mut(), v)
                .context(format!("invalid inspect size limit value for key {k}")),
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'protocol inspection config' should be 'map'"
        ))
    }
}
