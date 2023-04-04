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

use g3_tls_cert::agent::CertAgentConfig;

pub fn as_tls_cert_agent_config(value: &Yaml) -> anyhow::Result<CertAgentConfig> {
    if let Yaml::Hash(map) = value {
        let mut config = CertAgentConfig::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "cache_request_batch_count" => {
                let count = crate::value::as_usize(v)?;
                config.set_cache_request_batch_count(count);
                Ok(())
            }
            "cache_request_timeout" => {
                let time = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                config.set_cache_request_timeout(time);
                Ok(())
            }
            "cache_vanish_wait" | "vanish_after_expire" => {
                let time = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                config.set_cache_vanish_wait(time);
                Ok(())
            }
            "query_peer_addr" => {
                let addr = crate::value::as_sockaddr(v)
                    .context(format!("invalid sockaddr str value for key {k}"))?;
                config.set_query_peer_addr(addr);
                Ok(())
            }
            "query_socket_buffer" => {
                let buf_config = crate::value::as_socket_buffer_config(v)
                    .context(format!("invalid socket buffer config value for key {k}"))?;
                config.set_query_socket_buffer(buf_config);
                Ok(())
            }
            "query_wait_timeout" => {
                let time = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                config.set_query_wait_timeout(time);
                Ok(())
            }
            "protective_cache_ttl" => {
                let ttl = crate::value::as_u32(v)?;
                config.set_protective_cache_ttl(ttl);
                Ok(())
            }
            "maximum_cache_ttl" => {
                let ttl = crate::value::as_u32(v)?;
                config.set_maximum_cache_ttl(ttl);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml type for 'tls cert generator config' should be 'map'"
        ))
    }
}
