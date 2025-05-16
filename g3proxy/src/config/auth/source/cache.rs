/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};

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
                let user = UserConfig::parse_yaml(map, None)
                    .context(format!("invalid user config value for doc #{di}"))?;
                users.push(user);
            }
            Yaml::Array(seq) => {
                for (i, v) in seq.iter().enumerate() {
                    match v {
                        Yaml::Hash(map) => {
                            let user = UserConfig::parse_yaml(map, None).context(format!(
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
