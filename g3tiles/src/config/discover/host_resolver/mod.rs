/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;

use g3_types::metrics::NodeName;
use g3_types::net::UpstreamAddr;
use g3_yaml::YamlDocPosition;

use super::{
    AnyDiscoverConfig, CONFIG_KEY_DISCOVER_NAME, CONFIG_KEY_DISCOVER_TYPE, DiscoverConfig,
    DiscoverConfigDiffAction,
};

mod yaml;

const DISCOVER_CONFIG_TYPE: &str = "HostResolver";

pub(crate) struct HostResolverDiscoverInput {
    pub(crate) addr: UpstreamAddr,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct HostResolverDiscoverConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
}

impl HostResolverDiscoverConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        HostResolverDiscoverConfig {
            name: NodeName::default(),
            position,
        }
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        Ok(())
    }
}

impl DiscoverConfig for HostResolverDiscoverConfig {
    #[inline]
    fn name(&self) -> &NodeName {
        &self.name
    }

    #[inline]
    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    #[inline]
    fn r#type(&self) -> &'static str {
        DISCOVER_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyDiscoverConfig) -> DiscoverConfigDiffAction {
        let AnyDiscoverConfig::HostResolver(_new) = new else {
            return DiscoverConfigDiffAction::SpawnNew;
        };

        DiscoverConfigDiffAction::NoAction
    }
}
