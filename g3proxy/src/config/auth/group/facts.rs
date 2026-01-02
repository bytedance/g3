/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use yaml_rust::{Yaml, yaml};

use g3_yaml::YamlDocPosition;

use super::{BasicUserGroupConfig, UserGroupConfig};

const USER_GROUP_TYPE: &str = "facts";

#[derive(Clone)]
pub(crate) struct FactsUserGroupConfig {
    basic: BasicUserGroupConfig,
}

impl FactsUserGroupConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        FactsUserGroupConfig {
            basic: BasicUserGroupConfig::new(position),
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut config = Self::new(position);
        g3_yaml::foreach_kv(map, |k, v| config.set(k, v))?;
        config.check()?;
        Ok(config)
    }

    fn check(&self) -> anyhow::Result<()> {
        self.basic.check()
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        // no extra config keys defined for this user group
        self.basic.set(k, v)
    }
}

impl UserGroupConfig for FactsUserGroupConfig {
    fn basic_config(&self) -> &BasicUserGroupConfig {
        &self.basic
    }

    fn r#type(&self) -> &'static str {
        USER_GROUP_TYPE
    }
}
