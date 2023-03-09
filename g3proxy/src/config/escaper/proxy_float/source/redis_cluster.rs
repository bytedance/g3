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

use std::str::FromStr;
use std::time::Duration;

use anyhow::{anyhow, Context};
use yaml_rust::{yaml, Yaml};

use g3_types::net::UpstreamAddr;

const REDIS_DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProxyFloatRedisClusterSource {
    pub(crate) initial_nodes: Vec<UpstreamAddr>,
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
    pub(crate) read_timeout: Duration,
    pub(crate) sets_key: String,
}

impl ProxyFloatRedisClusterSource {
    fn new() -> Self {
        ProxyFloatRedisClusterSource {
            initial_nodes: Vec::new(),
            username: None,
            password: None,
            read_timeout: REDIS_DEFAULT_READ_TIMEOUT,
            sets_key: String::new(),
        }
    }

    pub(super) fn parse_map(map: &yaml::Hash) -> anyhow::Result<Self> {
        let mut config = ProxyFloatRedisClusterSource::new();

        g3_yaml::foreach_kv(map, |k, v| config.set(k, v))?;

        config.check()?;
        Ok(config)
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.initial_nodes.is_empty() {
            return Err(anyhow!("no initial nodes set"));
        }
        if self.sets_key.is_empty() {
            return Err(anyhow!("no sets name set"));
        }
        Ok(())
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_SOURCE_TYPE => Ok(()),
            "initial_nodes" | "startup_nodes" => match v {
                Yaml::String(addr) => self.add_initial_node(addr),
                Yaml::Array(seq) => {
                    for (i, v) in seq.iter().enumerate() {
                        if let Yaml::String(addr) = v {
                            self.add_initial_node(addr)?;
                        } else {
                            return Err(anyhow!("invalid string value for {k}#{i}"));
                        }
                    }
                    Ok(())
                }
                _ => Err(anyhow!("invalid value type for key {k}")),
            },
            "username" => {
                let username = g3_yaml::value::as_string(v)?;
                self.username = Some(username);
                Ok(())
            }
            "password" => {
                let password = g3_yaml::value::as_string(v)?;
                self.password = Some(password);
                Ok(())
            }
            "read_timeout" => {
                self.read_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "sets_key" => {
                self.sets_key = g3_yaml::value::as_string(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn add_initial_node(&mut self, addr: &str) -> anyhow::Result<()> {
        let upstream = UpstreamAddr::from_str(addr)?;
        self.initial_nodes.push(upstream);
        Ok(())
    }
}
