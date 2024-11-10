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

use anyhow::{anyhow, Context};
use yaml_rust::Yaml;

use crate::IpLocateServiceConfig;

impl IpLocateServiceConfig {
    fn set_query_peer_addr_by_yaml(&mut self, value: &Yaml) -> anyhow::Result<()> {
        let addr = g3_yaml::value::as_env_sockaddr(value)?;
        self.set_query_peer_addr(addr);
        Ok(())
    }

    pub fn parse_yaml(value: &Yaml) -> anyhow::Result<Self> {
        match value {
            Yaml::Hash(map) => {
                let mut config = IpLocateServiceConfig::default();

                g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                    "cache_request_batch_count" => {
                        let count = g3_yaml::value::as_usize(v)?;
                        config.set_cache_request_batch_count(count);
                        Ok(())
                    }
                    "cache_request_timeout" => {
                        let time = g3_yaml::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_cache_request_timeout(time);
                        Ok(())
                    }
                    "query_peer_addr" => {
                        config
                            .set_query_peer_addr_by_yaml(v)
                            .context(format!("invalid sockaddr str value for key {k}"))?;
                        Ok(())
                    }
                    "query_socket_buffer" => {
                        let buf_config = g3_yaml::value::as_socket_buffer_config(v)
                            .context(format!("invalid socket buffer config value for key {k}"))?;
                        config.set_query_socket_buffer(buf_config);
                        Ok(())
                    }
                    "query_wait_timeout" => {
                        let time = g3_yaml::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_query_wait_timeout(time);
                        Ok(())
                    }
                    "default_expire_ttl" => {
                        let ttl = g3_yaml::value::as_u32(v)?;
                        config.set_default_expire_ttl(ttl);
                        Ok(())
                    }
                    "maximum_expire_ttl" => {
                        let ttl = g3_yaml::value::as_u32(v)?;
                        config.set_maximum_expire_ttl(ttl);
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid key {k}")),
                })?;

                Ok(config)
            }
            Yaml::String(_) => {
                let mut config = IpLocateServiceConfig::default();
                config
                    .set_query_peer_addr_by_yaml(value)
                    .context("invalid sockaddr str value")?;
                Ok(config)
            }
            _ => Err(anyhow!(
                "yaml type for 'ip location service config' should be 'map'"
            )),
        }
    }
}
