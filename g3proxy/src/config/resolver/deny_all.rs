/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeSet;

use anyhow::anyhow;
use yaml_rust::{Yaml, yaml};

use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use super::{AnyResolverConfig, ResolverConfig, ResolverConfigDiffAction};

const RESOLVER_CONFIG_TYPE: &str = "deny-all";

#[derive(Clone)]
pub(crate) struct DenyAllResolverConfig {
    position: Option<YamlDocPosition>,
    name: NodeName,
}

impl DenyAllResolverConfig {
    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut resolver = DenyAllResolverConfig {
            position,
            name: NodeName::default(),
        };

        g3_yaml::foreach_kv(map, |k, v| resolver.set(k, v))?;

        resolver.check()?;
        Ok(resolver)
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }

        Ok(())
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_RESOLVER_TYPE => Ok(()),
            super::CONFIG_KEY_RESOLVER_NAME => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}

impl ResolverConfig for DenyAllResolverConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn r#type(&self) -> &'static str {
        RESOLVER_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyResolverConfig) -> ResolverConfigDiffAction {
        let AnyResolverConfig::DenyAll(_new) = new else {
            return ResolverConfigDiffAction::SpawnNew;
        };

        ResolverConfigDiffAction::NoAction
    }

    fn dependent_resolver(&self) -> Option<BTreeSet<NodeName>> {
        None
    }
}
