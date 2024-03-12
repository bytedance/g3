/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use anyhow::anyhow;

use g3_types::metrics::MetricsName;
use g3_types::net::UpstreamAddr;
use g3_yaml::YamlDocPosition;

use super::{
    AnyDiscoverConfig, DiscoverConfig, DiscoverConfigDiffAction, CONFIG_KEY_DISCOVER_NAME,
    CONFIG_KEY_DISCOVER_TYPE,
};

mod yaml;

const DISCOVER_CONFIG_TYPE: &str = "HostResolver";

pub(crate) struct HostResolverDiscoverInput {
    pub(crate) addr: UpstreamAddr,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct HostResolverDiscoverConfig {
    name: MetricsName,
    position: Option<YamlDocPosition>,
}

impl HostResolverDiscoverConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        HostResolverDiscoverConfig {
            name: MetricsName::default(),
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
    fn name(&self) -> &MetricsName {
        &self.name
    }

    #[inline]
    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    #[inline]
    fn discover_type(&self) -> &'static str {
        DISCOVER_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyDiscoverConfig) -> DiscoverConfigDiffAction {
        let AnyDiscoverConfig::HostResolver(_new) = new else {
            return DiscoverConfigDiffAction::SpawnNew;
        };

        DiscoverConfigDiffAction::NoAction
    }
}
