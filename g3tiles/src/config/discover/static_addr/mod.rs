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

use std::net::SocketAddr;

use anyhow::anyhow;

use g3_types::collection::WeightedValue;
use g3_types::metrics::MetricsName;
use g3_yaml::YamlDocPosition;

use super::{
    AnyDiscoverConfig, DiscoverConfig, DiscoverConfigDiffAction, CONFIG_KEY_DISCOVER_NAME,
    CONFIG_KEY_DISCOVER_TYPE,
};

mod yaml;

const DISCOVER_CONFIG_TYPE: &str = "StaticAddr";

#[derive(Default, PartialEq, Eq)]
pub(crate) struct StaticAddrDiscoverInput {
    pub(crate) inner: Vec<WeightedValue<SocketAddr>>,
}

#[derive(Clone)]
pub(crate) struct StaticAddrDiscoverConfig {
    name: MetricsName,
    position: Option<YamlDocPosition>,
}

impl StaticAddrDiscoverConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        StaticAddrDiscoverConfig {
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

impl DiscoverConfig for StaticAddrDiscoverConfig {
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
        let AnyDiscoverConfig::StaticAddr(_new) = new else {
            return DiscoverConfigDiffAction::SpawnNew;
        };

        DiscoverConfigDiffAction::NoAction
    }
}
