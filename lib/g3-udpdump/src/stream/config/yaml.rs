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

use super::StreamDumpConfig;

impl StreamDumpConfig {
    pub fn parse_yaml(value: &Yaml) -> anyhow::Result<Self> {
        match value {
            Yaml::Hash(map) => {
                let mut config = StreamDumpConfig::default();

                g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                    "peer" => {
                        config.peer = g3_yaml::value::as_env_sockaddr(v)?;
                        Ok(())
                    }
                    "socket_buffer" => {
                        config.buffer = g3_yaml::value::as_socket_buffer_config(v)
                            .context(format!("invalid socket buffer config value for key {k}"))?;
                        Ok(())
                    }
                    "misc_opts" => {
                        config.opts = g3_yaml::value::as_udp_misc_sock_opts(v)
                            .context(format!("invalid udp misc socket option value for key {k}"))?;
                        Ok(())
                    }
                    "packet_size" => {
                        config.packet_size = g3_yaml::value::as_usize(v)?;
                        Ok(())
                    }
                    "client_side" => {
                        config.client_side = g3_yaml::value::as_bool(v)?;
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid key {k}")),
                })?;

                Ok(config)
            }
            Yaml::String(_) => {
                let config = StreamDumpConfig {
                    peer: g3_yaml::value::as_env_sockaddr(value)?,
                    ..Default::default()
                };
                Ok(config)
            }
            _ => Err(anyhow!(
                "yaml type for 'stream dump config' should be 'map'"
            )),
        }
    }
}
