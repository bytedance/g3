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

use std::net::SocketAddr;

use anyhow::{anyhow, Context};
use yaml_rust::Yaml;

use g3_types::collection::{SelectivePickPolicy, WeightedValue};
use g3_yaml::{YamlDocPosition, YamlMapCallback};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OpensslServiceConfig {
    pub(crate) addrs: Vec<WeightedValue<SocketAddr>>,
    pub(crate) pick_policy: SelectivePickPolicy,
}

impl Default for OpensslServiceConfig {
    fn default() -> Self {
        OpensslServiceConfig {
            addrs: Vec::new(),
            pick_policy: SelectivePickPolicy::Random,
        }
    }
}

impl YamlMapCallback for OpensslServiceConfig {
    fn type_name(&self) -> &'static str {
        "OpensslServiceConfig"
    }

    fn parse_kv(
        &mut self,
        key: &str,
        value: &Yaml,
        _doc: Option<&YamlDocPosition>,
    ) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(key).as_str() {
            "addrs" => {
                if let Yaml::Array(seq) = value {
                    for (i, v) in seq.iter().enumerate() {
                        let addr = g3_yaml::value::as_weighted_sockaddr(v).context(format!(
                            "invalid weighted sockaddr string value for {key}#{i}"
                        ))?;
                        self.addrs.push(addr);
                    }
                } else {
                    let addr = g3_yaml::value::as_weighted_sockaddr(value)
                        .context(format!("invalid weighted sockaddr string value for {key}"))?;
                    self.addrs.push(addr);
                }
                Ok(())
            }
            "pick_policy" => {
                self.pick_policy = g3_yaml::value::as_selective_pick_policy(value)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {key}")),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.addrs.is_empty() {
            return Err(anyhow!("no address set"));
        }
        Ok(())
    }
}
