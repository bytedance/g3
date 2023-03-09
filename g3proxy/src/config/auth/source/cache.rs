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

use crate::config::auth::UserConfig;

pub(crate) fn parse_json(value: &serde_json::Value) -> anyhow::Result<Vec<UserConfig>> {
    use serde_json::Value;

    let mut users = Vec::new();
    match value {
        Value::Array(seq) => {
            for (i, v) in seq.iter().enumerate() {
                match v {
                    Value::Object(map) => {
                        let user = UserConfig::parse_json(map)
                            .context(format!("invalid user config value for record #{i}"))?;
                        users.push(user);
                    }
                    _ => return Err(anyhow!("invalid value type for record #{i}")),
                }
            }
        }
        _ => return Err(anyhow!("invalid root value type")),
    }
    Ok(users)
}

pub(crate) fn parse_yaml(docs: &[yaml_rust::Yaml]) -> anyhow::Result<Vec<UserConfig>> {
    use yaml_rust::Yaml;

    let mut users = Vec::new();
    for (di, doc) in docs.iter().enumerate() {
        match doc {
            Yaml::Hash(map) => {
                let user = UserConfig::parse_yaml(map)
                    .context(format!("invalid user config value for doc #{di}"))?;
                users.push(user);
            }
            Yaml::Array(seq) => {
                for (i, v) in seq.iter().enumerate() {
                    match v {
                        Yaml::Hash(map) => {
                            let user = UserConfig::parse_yaml(map).context(format!(
                                "invalid user config value for doc #{di} record #{i}"
                            ))?;
                            users.push(user);
                        }
                        _ => return Err(anyhow!("invalid value type for doc #{di} record #{i}")),
                    }
                }
            }
            _ => return Err(anyhow!("invalid value type for doc #{di}")),
        }
    }
    Ok(users)
}
