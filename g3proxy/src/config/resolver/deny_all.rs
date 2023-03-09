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

use std::collections::BTreeSet;

use anyhow::anyhow;
use yaml_rust::{yaml, Yaml};

use g3_yaml::YamlDocPosition;

use super::{AnyResolverConfig, ResolverConfig, ResolverConfigDiffAction};

const RESOLVER_CONFIG_TYPE: &str = "deny-all";

#[derive(Clone)]
pub(crate) struct DenyAllResolverConfig {
    position: Option<YamlDocPosition>,
    name: String,
}

impl DenyAllResolverConfig {
    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let name = g3_yaml::hash_get_required_str(map, super::CONFIG_KEY_RESOLVER_NAME)?;
        let mut resolver = DenyAllResolverConfig {
            position,
            name: name.to_string(),
        };

        g3_yaml::foreach_kv(map, |k, v| resolver.set(k, v))?;

        Ok(resolver)
    }

    fn set(&mut self, k: &str, _v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_RESOLVER_TYPE => Ok(()),
            super::CONFIG_KEY_RESOLVER_NAME => Ok(()),
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}

impl ResolverConfig for DenyAllResolverConfig {
    fn name(&self) -> &str {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn resolver_type(&self) -> &'static str {
        RESOLVER_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyResolverConfig) -> ResolverConfigDiffAction {
        match new {
            AnyResolverConfig::DenyAll(_) => ResolverConfigDiffAction::NoAction,
            _ => ResolverConfigDiffAction::SpawnNew,
        }
    }

    fn dependent_resolver(&self) -> Option<BTreeSet<String>> {
        None
    }
}
