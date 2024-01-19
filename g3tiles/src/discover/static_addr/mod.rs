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

use std::sync::Arc;

use anyhow::Context;
use tokio::sync::watch;
use yaml_rust::Yaml;

use super::{ArcDiscover, Discover, DiscoverResult};
use crate::config::discover::static_addr::StaticAddrDiscoverConfig;
use crate::config::discover::{AnyDiscoverConfig, DiscoverConfig};

pub(crate) struct StaticAddrDiscover {
    config: StaticAddrDiscoverConfig,
}

impl StaticAddrDiscover {
    pub(crate) fn new_obj(config: StaticAddrDiscoverConfig) -> ArcDiscover {
        Arc::new(StaticAddrDiscover { config })
    }
}

impl Discover for StaticAddrDiscover {
    fn _clone_config(&self) -> AnyDiscoverConfig {
        AnyDiscoverConfig::StaticAddr(self.config.clone())
    }

    fn _update_config_in_place(&self, _config: AnyDiscoverConfig) -> anyhow::Result<()> {
        Ok(())
    }

    fn register_yaml(&self, data: &Yaml) -> anyhow::Result<watch::Receiver<DiscoverResult>> {
        let input = self.config.parse_yaml_data(data).context(format!(
            "invalid input data for discover {}",
            self.config.name()
        ))?;
        let (sender, mut receiver) = watch::channel(Ok(input.inner));
        receiver.mark_changed();
        tokio::spawn(async move { sender.closed().await });
        Ok(receiver)
    }
}
