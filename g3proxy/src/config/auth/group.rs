/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow};
use arcstr::ArcStr;
use yaml_rust::{Yaml, yaml};

use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use crate::config::auth::{UserConfig, UserDynamicSource};

const DEFAULT_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub(crate) struct UserGroupConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) static_users: HashMap<ArcStr, Arc<UserConfig>>,
    pub(crate) dynamic_source: Option<UserDynamicSource>,
    pub(crate) dynamic_cache: PathBuf,
    pub(crate) refresh_interval: Duration,
    pub(crate) anonymous_user: Option<Arc<UserConfig>>,
}

impl UserGroupConfig {
    pub(crate) fn name(&self) -> &NodeName {
        &self.name
    }

    pub(crate) fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    pub(crate) fn empty(name: &NodeName) -> Self {
        UserGroupConfig {
            name: name.clone(),
            position: None,
            static_users: HashMap::new(),
            dynamic_source: None,
            dynamic_cache: PathBuf::default(),
            refresh_interval: DEFAULT_REFRESH_INTERVAL,
            anonymous_user: None,
        }
    }

    pub(crate) fn new(position: Option<YamlDocPosition>) -> Self {
        UserGroupConfig {
            name: NodeName::default(),
            position,
            static_users: HashMap::new(),
            dynamic_source: None,
            dynamic_cache: PathBuf::default(),
            refresh_interval: DEFAULT_REFRESH_INTERVAL,
            anonymous_user: None,
        }
    }

    pub(crate) fn parse(&mut self, map: &yaml::Hash) -> anyhow::Result<()> {
        g3_yaml::foreach_kv(map, |k, v| self.set(k, v))?;
        self.check()?;
        Ok(())
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }

        Ok(())
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "name" => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "static_users" => {
                if let Yaml::Array(seq) = v {
                    for (i, obj) in seq.iter().enumerate() {
                        if let Yaml::Hash(map) = obj {
                            let user =
                                Arc::new(UserConfig::parse_yaml(map, self.position.as_ref())?);
                            let username = user.name().clone();
                            if let Some(old) = self.static_users.insert(username, user) {
                                return Err(anyhow!(
                                    "found duplicate entry for user {}",
                                    old.name()
                                ));
                            }
                        } else {
                            return Err(anyhow!("invalid hash value for key {k}#{i}"));
                        }
                    }
                    Ok(())
                } else {
                    Err(anyhow!("invalid sequence value for key {k}"))
                }
            }
            "source" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(self.position.as_ref())?;
                self.dynamic_source = Some(
                    UserDynamicSource::parse_config(v, lookup_dir)
                        .context(format!("invalid value for key {k}"))?,
                );
                Ok(())
            }
            "cache" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(self.position.as_ref())?;
                self.dynamic_cache = g3_yaml::value::as_file_path(v, lookup_dir, true)
                    .context(format!("invalid file path value for key {k}"))?;
                Ok(())
            }
            "refresh_interval" => {
                self.refresh_interval = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid duration value for key {k}"))?;
                Ok(())
            }
            "anonymous_user" => {
                if let Yaml::Hash(map) = v {
                    let mut user = UserConfig::parse_yaml(map, self.position.as_ref())?;
                    user.set_no_password();
                    self.anonymous_user = Some(Arc::new(user));
                    Ok(())
                } else {
                    Err(anyhow!("invalid hash value for key {k}"))
                }
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}
