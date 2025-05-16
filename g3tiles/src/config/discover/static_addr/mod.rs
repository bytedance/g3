/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;

use anyhow::anyhow;

use g3_types::collection::WeightedValue;
use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use super::{
    AnyDiscoverConfig, CONFIG_KEY_DISCOVER_NAME, CONFIG_KEY_DISCOVER_TYPE, DiscoverConfig,
    DiscoverConfigDiffAction,
};

mod yaml;

const DISCOVER_CONFIG_TYPE: &str = "StaticAddr";

#[derive(Default, PartialEq, Eq)]
pub(crate) struct StaticAddrDiscoverInput {
    pub(crate) inner: Vec<WeightedValue<SocketAddr>>,
}

#[derive(Clone)]
pub(crate) struct StaticAddrDiscoverConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
}

impl StaticAddrDiscoverConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        StaticAddrDiscoverConfig {
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

impl DiscoverConfig for StaticAddrDiscoverConfig {
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
        let AnyDiscoverConfig::StaticAddr(_new) = new else {
            return DiscoverConfigDiffAction::SpawnNew;
        };

        DiscoverConfigDiffAction::NoAction
    }
}
